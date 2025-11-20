use std::time::Duration;

use alloy::{hex, primitives::Bytes};
use alloy_json_abi::Function;
use eyre::eyre;
use reqwest::Client;
use serde::Deserialize;

// #[derive(Deserialize)] lets serde_json auto-parse the API response
#[derive(Debug, Deserialize)]
struct FourByteResponse {
    results: Vec<FourByteSignature>,
}

#[derive(Debug, Deserialize)]
struct FourByteSignature {
    text_signature: String,
}

// Constants
pub const WELL_KNOWN_FUNC_NAME: [&str; 6] = [
    "transfer",
    "approve",
    "transferFrom",
    "mint",
    "burn",
    "swap",
];

/// Extracts the first four bytes from the given byte slice to use as a function selector.
pub fn selector(data: &Bytes) -> eyre::Result<[u8; 4]> {
    data.get(..4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| eyre!("data too short to extract selector"))
}

/// Looks up the given function selector on the 4byte.directory API and returns a list of matching
/// function signatures.
pub async fn lookup_selector(selector: [u8; 4]) -> eyre::Result<Vec<String>> {
    let hex_sig = format!("0x{}", hex::encode(selector));
    let url = format!(
        "https://www.4byte.directory/api/v1/signatures/?hex_signature={}",
        hex_sig
    );

    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

    let response: FourByteResponse = client.get(&url).send().await?.json().await?;

    Ok(response
        .results
        .into_iter()
        .map(|r| r.text_signature)
        .collect())
}

/// Parses a function signature string (e.g., "transfer(address,uint256)")
/// into an Alloy Function that can decode calldata.
pub fn parse_signature(sig: &str) -> eyre::Result<Function> {
    // alloys built-in parser for Solidity signatures
    Function::parse(sig).map_err(|e| eyre!("failed to parse signature '{}': {}", sig, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_extraction() {
        let data = Bytes::from(vec![0xa9, 0x05, 0x9c, 0xbb, 0x00, 0x00]);
        let sel = selector(&data).unwrap();
        assert_eq!(sel, [0xa9, 0x05, 0x9c, 0xbb]);
    }

    #[test]
    fn test_selector_too_short() {
        let data = Bytes::from(vec![0xa9, 0x05]);
        let result = selector(&data);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "data too short to extract selector"
        );
    }

    #[test]
    fn test_parse_signature() {
        let func = parse_signature("transfer(address,uint256)").unwrap();
        assert_eq!(func.name, "transfer");
        assert_eq!(func.inputs.len(), 2);
    }

    #[tokio::test]
    async fn test_lookup_selector() {
        let sel = [0xa9, 0x05, 0x9c, 0xbb]; // transfer(address,uint256)
        let sigs = lookup_selector(sel).await.unwrap();
        assert!(!sigs.is_empty());
        assert!(sigs.iter().any(|s| s.contains("transfer")));
    }
}
