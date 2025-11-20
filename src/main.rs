use std::{env, fs, path::PathBuf, time::Duration};

use alloy::{
    dyn_abi::{DynSolValue, JsonAbiExt},
    hex,
    primitives::Bytes,
};
use alloy_json_abi::Function;
use clap::Parser;
use comfy_table::{Attribute, Cell, Color, Table};
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

#[derive(Parser, Debug)]
#[command(name = "txdecode", about = "Decode Ethereum transaction calldata", long_about = None)]
struct Args {
    /// Transaction hash to decode (fetch from RPC)
    #[arg(value_name = "TX_HASH")]
    tx_hash: Option<String>,

    /// Raw calldata in hex format (overrides TX_HASH)
    #[arg(short, long, value_name = "CALDATA")]
    input: Option<String>,

    /// RPC endpoint URL
    #[arg(long, default_value = "https://ethereum-rpc.publicnode.com")]
    rpc: String,

    /// Etherscan API key for ABI fetching
    #[arg(long, env = "ETHERSCAN_API_KEY")]
    etherscan_key: Option<String>,
}

/// Returns the path to the cache directory, creating it if it doesn't exist.
fn cache_dir() -> eyre::Result<PathBuf> {
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE"))?;
    let cache = PathBuf::from(home).join(".txdecode").join("cache");
    fs::create_dir_all(&cache)?;
    Ok(cache)
}

/// Returns the cache file path for a given contract address.
fn cache_path(address: &str) -> eyre::Result<PathBuf> {
    Ok(cache_dir()?.join(format!("{}.json", address.to_lowercase())))
}

/// Loads the cached ABI for the given contract address, if it exists.
fn load_cache_abi(address: &str) -> Option<Vec<Function>> {
    let path = cache_path(address).ok()?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Saves the given ABI to the cache for the specified contract address.
fn save_cached_abi(address: &str, abi: &[Function]) -> eyre::Result<()> {
    let path = cache_path(address)?;
    let json = serde_json::to_string_pretty(abi)?;
    fs::write(path, json)?;
    Ok(())
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
    // Check cache first
    if let Some(cached_abi) = load_cache_abi(contract_address) {
        if let Some(func) = cached_abi.iter().find(|f| f.selector() == selector) {
            return Ok(func.clone());
        }
    }

    // Fetch from Etherscan
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

    // Cache the ABI for future use
    save_cached_abi(contract_address, &abi)?;

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
) -> eyre::Result<(Function, Vec<DynSolValue>)> {
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
                return Ok((func, decoded));
            }
        }
    }

    // Fallback to Etherscan ABI if contract address and API key are provided
    if let (Some(addr), Some(key)) = (contract_address, etherscan_key) {
        let func = fetch_etherscan_abi(addr, sel, key).await?;
        let decoded = try_decode(&func, calldata)?;
        return Ok((func, decoded));
    }

    bail!(
        "all {} signatures failed to decode calldata",
        signatures.len()
    )
}

/// Formats a DynSolValue into a human-readable string.
fn format_value(value: &DynSolValue) -> String {
    match value {
        DynSolValue::Address(addr) => {
            // Format checksum address
            let add_str = format!("{:?}", addr);

            // Check for well-known address
            if addr.is_zero() {
                format!("{} (Zero Address)", add_str)
            } else {
                add_str
            }
        }
        DynSolValue::Uint(val, bits) => {
            // Format bigint with underscores
            let num_str = val.to_string();
            if num_str.len() > 6 {
                // Insert underscores every 3 digits from the right
                let formatted = num_str
                    .chars()
                    .rev()
                    .collect::<Vec<_>>()
                    .chunks(3)
                    .map(|chunk| chunk.iter().collect::<String>())
                    .collect::<Vec<_>>()
                    .join("_")
                    .chars()
                    .rev()
                    .collect::<String>();
                format!("{} (uint{})", formatted, bits)
            } else {
                format!("{} (uint{})", num_str, bits)
            }
        }
        DynSolValue::Bool(b) => format!("{}", b),
        DynSolValue::Bytes(bytes) => {
            if bytes.len() <= 32 {
                format!("0x{}", hex::encode(bytes))
            } else {
                format!("0x{}... ({} bytes)", hex::encode(&bytes[..32]), bytes.len())
            }
        }
        _ => format!("{:?}", value),
    }
}

/// Displays the decoded function name and parameters in a formatted table.
fn display_decoded(func_name: &str, params: &[DynSolValue], func: &Function) {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Parameter")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
        Cell::new("Type")
            .fg(Color::Yellow)
            .add_attribute(Attribute::Bold),
        Cell::new("Value")
            .fg(Color::Green)
            .add_attribute(Attribute::Bold),
    ]);

    // Zip parameters with their types from the function ABI
    for (i, (param, input)) in params.iter().zip(&func.inputs).enumerate() {
        table.add_row(vec![
            Cell::new(if input.name.is_empty() {
                format!("param{}", i)
            } else {
                input.name.clone()
            }),
            Cell::new(input.ty.to_string()).fg(Color::Yellow),
            Cell::new(format_value(param)).fg(Color::White),
        ]);
    }

    println!("\n✅ Function: {}", func_name);
    println!("{}", table);
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // for better error reporting
    color_eyre::install()?;
    let args = Args::parse();

    // Decode raw calldata if --input is provided
    if let Some(input_hex) = args.input {
        let calldata = hex::decode(input_hex.trim_start_matches("0x"))?;
        let bytes = Bytes::from(calldata);

        match decode_calldata(&bytes, None, args.etherscan_key.as_deref()).await {
            Ok((func, params)) => display_decoded(&func.name, &params, &func),
            Err(e) => println!("❌ Failed to decode calldata: {}", e),
        }
        return Ok(());
    }

    // TODO: Fetch transaction by hash (Step 10)
    if let Some(_tx_hash) = args.tx_hash {
        eprintln!("⚠️  Transaction decoding not yet implemented. Use --input for now.");
        return Ok(());
    }

    eprintln!("❌ Error: Provide either a transaction hash or --input <calldata>");
    Ok(())
}
