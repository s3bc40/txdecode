use std::{env, time::Duration};

use alloy::{
    dyn_abi::{DynSolValue, JsonAbiExt},
    hex,
    primitives::Bytes,
};
use alloy_json_abi::Function;
use eyre::{bail, eyre};
use reqwest::Client;
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

#[derive(Debug, Deserialize)]
struct EtherscanResponse {
    status: String,
    result: String,
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

/// Fetches the ABI from Etherscan for the given contract address and looks for a function
/// matching the provided selector.
async fn fetch_etherscan_abi(
    contract_address: &str,
    selector: [u8; 4],
    api_key: &str,
) -> eyre::Result<Function> {
    let url = format!(
        "https://api.etherscan.io/v2/api?module=contract&action=getabi&address={}&apikey={}",
        contract_address, api_key
    );

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let response: EtherscanResponse = client.get(&url).send().await?.json().await?;

    if response.status != "1" {
        bail!("failed to fetch ABI from Etherscan: {}", response.result);
    }

    let abi: Vec<Function> = serde_json::from_str(&response.result)
        .map_err(|e| eyre!("failed to parse ABI JSON: {}", e))?;

    abi.into_iter()
        .find(|f| f.selector() == selector)
        .ok_or_else(|| {
            eyre!(
                "function with selector 0x{} not found in ABI",
                hex::encode(selector)
            )
        })
}

/// Decodes the given calldata by looking up possible function signatures and trying to decode
/// with each until one succeeds.
async fn decode_calldata(
    calldata: &Bytes,
    contract_address: Option<&str>, // Optional
    etherscan_key: Option<&str>,    // Optional
) -> eyre::Result<(String, Vec<DynSolValue>)> {
    let sel = selector(calldata)?;
    let signatures = lookup_selector(sel).await?;

    if signatures.is_empty() {
        bail!("no signatures found for selector 0x{}", hex::encode(sel));
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

    // Fallback to Etherscan ABI if contract address and API key are provided
    if let (Some(addr), Some(key)) = (contract_address, etherscan_key) {
        let func = fetch_etherscan_abi(addr, sel, key).await?;
        let decoded = try_decode(&func, calldata)?;
        return Ok((func.name.clone(), decoded));
    }

    bail!(
        "all {} signatures failed to decode calldata",
        signatures.len()
    )
}

/// Test function to demonstrate decoding
async fn test_decode(calldata: &Bytes, contract: Option<&str>, api_key: Option<&str>) {
    println!("Decoding calldata ({} bytes)...\n", calldata.len());

    match decode_calldata(&calldata, contract, api_key).await {
        Ok((func_name, params)) => {
            println!("✅ Decoded using function: {}", func_name);
            println!("Parameters:");
            for (i, param) in params.iter().enumerate() {
                println!("  [{}]: {:?}", i, param);
            }
        }
        Err(e) => println!("❌ Failed to decode calldata: {}", e),
    }
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // for better error reporting
    color_eyre::install()?;

    // Test 1: Simple ERC-20 transfer
    println!("=== Test 1: ERC-20 transfer (4byte lookup) ===\n");
    let calldata = hex::decode(
        "a9059cbb0000000000000000000000000742d35cc6634c0532925a3b844bc9e7595f0beb00000000000000000000000000000000000000000000000000000000000f4240",
    )?;
    test_decode(&Bytes::from(calldata), None, None).await;

    println!("\n{}\n", "=".repeat(60));

    // Test 2: Custom contract function (requires Etherscan fallback)
    // Using a less common function that might not be in 4byte
    println!("=== Test 2: Custom function (Etherscan fallback) ===\n");
    let custom_calldata =
        hex::decode("12345678000000000000000000000000000000000000000000000000000000000000002a")?;

    let etherscan_key = env::var("ETHERSCAN_API_KEY").ok();
    let contract = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"; // Uniswap V3 Router

    test_decode(
        &Bytes::from(custom_calldata),
        Some(contract),
        etherscan_key.as_deref(),
    )
    .await;

    Ok(())
}
