use std::time::Duration;

use alloy::{dyn_abi::DynSolValue, hex, primitives::Bytes};
use alloy_json_abi::Function;
use eyre::{Ok, eyre};
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

/// Extracts the first four bytes from the given byte slice to use as a function selector.
fn _selector(data: &Bytes) -> eyre::Result<[u8; 4]> {
    data.get(..4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| eyre!("data too short to extract selector"))
}

/// Looks up the given function selector on the 4byte.directory API and returns a list of matching
/// function signatures.
async fn lookup_selector(selector: [u8; 4]) -> eyre::Result<Vec<String>> {
    let hex_sig = format!("0x{}", hex::encode(selector));
    let url = format!(
        "https://www.4byte.directory/api/v1/signatures/?hex_signature={}",
        hex_sig
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response: FourByteResponse = client.get(&url).send().await?.json().await?;

    Ok(response
        .results
        .into_iter()
        .map(|r| r.text_signature)
        .collect())
}

/// Parses a function signature string (e.g., "transfer(address,uint256)")
/// into an Alloy Function that can decode calldata.
fn parse_signature(sig: &str) -> eyre::Result<Function> {
    // alloys built-in parser for Solidity signatures
    Function::parse(sig).map_err(|e| eyre!("Failed to parse signature '{}': {}", sig, e))
}

fn try_decode(func: &Function, calldata: &Bytes) -> eyre::Result<Vec<DynSolValue>> {
    todo!()
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // for better error reporting
    color_eyre::install()?;

    // Example with transfer function selector
    let test_selector = [0xa9, 0x05, 0x9c, 0xbb];

    println!("Looking up selector: 0x{}", hex::encode(test_selector));
    let signatures = lookup_selector(test_selector).await?;

    println!("Found {} signatures:", signatures.len());
    for sig in &signatures {
        println!(" - {}", sig);
    }

    Ok(())
}
