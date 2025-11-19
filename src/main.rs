use std::time::Duration;

use alloy::{
    dyn_abi::{DynSolValue, JsonAbiExt},
    hex,
    primitives::Bytes,
};
use alloy_json_abi::Function;
use eyre::{bail, eyre};
use serde::Deserialize;

// Constants
const WELL_KNOWN_FUNC_NAME: [&str; 6] = [
    "transfer",
    "approve",
    "transferFrom",
    "mint",
    "burn",
    "swap",
];

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
fn selector(data: &Bytes) -> eyre::Result<[u8; 4]> {
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
    Function::parse(sig).map_err(|e| eyre!("failed to parse signature '{}': {}", sig, e))
}

/// Attempts to decode the given calldata using the provided Alloy Function.
/// DynSolValue is a dynamic representation of Solidity values (not at compile time).
fn try_decode(func: &Function, calldata: &Bytes) -> eyre::Result<Vec<DynSolValue>> {
    // Skip the first 4 bytes (the function selector)
    let params = calldata
        .get(4..)
        .ok_or_else(|| eyre!("calldata missing parameters"))?;

    // Decode the parameters using the function's input ABI
    let decoded = func
        .abi_decode_input(params)
        .map_err(|e| eyre!("failed to decode: {}", e))?;

    Ok(decoded)
}

/// Decodes the given calldata by looking up possible function signatures and trying to decode
/// with each until one succeeds.
async fn decode_calldata(calldata: &Bytes) -> eyre::Result<(String, Vec<DynSolValue>)> {
    let selector = selector(calldata)?;
    let signatures = lookup_selector(selector).await?;

    if signatures.is_empty() {
        bail!(
            "no signatures found for selector 0x{}",
            hex::encode(selector)
        );
    }

    // Priroritize common signatures (e.g., ERC-20 transfer)
    let mut prioritized: Vec<&String> = signatures
        .iter()
        .filter(|s| WELL_KNOWN_FUNC_NAME.iter().any(|wk| s.starts_with(wk)))
        .collect();

    // Append the rest of the signatures
    prioritized.extend(
        signatures
            .iter()
            .filter(|s| !WELL_KNOWN_FUNC_NAME.iter().any(|wk| s.starts_with(wk))),
    );

    // Try to decode using each signature until one works
    for sig in prioritized {
        if let Ok(func) = parse_signature(sig) {
            if let Ok(decoded) = try_decode(&func, calldata) {
                return Ok((func.name.clone(), decoded));
            }
        }
    }

    bail!(
        "all {} signatures failed to decode calldata",
        signatures.len()
    )
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // for better error reporting
    color_eyre::install()?;

    // Real ERC-20 transfer calldata: transfer(address,uint256)
    // Selector: 0xa9059cbb
    // to: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
    // amount: 1000000 (1 USDC with 6 decimals)
    let calldata = hex::decode(
        "a9059cbb0000000000000000000000000742d35cc6634c0532925a3b844bc9e7595f0beb00000000000000000000000000000000000000000000000000000000000f4240",
    )?;
    let calldata = Bytes::from(calldata);

    println!("Decoding calldata ({} bytes)...\n", calldata.len());

    // Attempt to decode the calldata
    match decode_calldata(&calldata).await {
        Ok((func_name, params)) => {
            println!("✅ Decoded using function: {}", func_name);
            println!("Parameters:");
            for (i, param) in params.iter().enumerate() {
                println!("  [{}]: {:?}", i, param);
            }
        }
        Err(e) => println!("❌ Failed to decode calldata: {}", e),
    }

    Ok(())
}
