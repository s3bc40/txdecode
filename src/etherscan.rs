use std::time::Duration;

use alloy::hex;
use alloy_json_abi::{Function, JsonAbi};
use eyre::{bail, eyre};
use reqwest::Client;
use serde::Deserialize;

use crate::cache;

#[derive(Debug, Deserialize)]
struct EtherscanResponse {
    status: String,
    result: String,
}

/// Fetches the ABI from Etherscan for the given contract address and looks for a function
/// matching the provided selector.
pub async fn fetch_etherscan_abi(
    contract_address: &str,
    selector: [u8; 4],
    api_key: &str,
    chain_id: Option<u32>,
) -> eyre::Result<Function> {
    // Check cache first
    if let Some(cached_abi) = cache::load_cache_abi(contract_address) {
        if let Some(func) = cached_abi.iter().find(|f| f.selector() == selector) {
            return Ok(func.clone());
        }
    }

    // Fetch from Etherscan
    let chain = chain_id.unwrap_or(1);
    let url = format!(
        "https://api.etherscan.io/v2/api?module=contract&action=getabi&address={}&apikey={}&chainid={}",
        contract_address, api_key, chain
    );

    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let response: EtherscanResponse = client.get(&url).send().await?.json().await?;

    if response.status != "1" {
        bail!("failed to fetch ABI from Etherscan: {}", response.result);
    }

    let full_abi: JsonAbi = serde_json::from_str(&response.result)
        .map_err(|e| eyre!("failed to parse ABI JSON: {}", e))?;

    let functions: Vec<Function> = full_abi.functions().cloned().collect();

    // Cache the ABI for future use
    cache::save_cached_abi(contract_address, &functions)?;

    functions
        .into_iter()
        .find(|f| f.selector() == selector)
        .ok_or_else(|| {
            eyre!(
                "function with selector 0x{} not found in ABI",
                hex::encode(selector)
            )
        })
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[tokio::test]
    #[ignore] // Requires a valid Etherscan API key
    async fn test_fetch_etherscan_abi() {
        let api_key = env::var("ETHERSCAN_API_KEY").unwrap();
        // USDT contract
        let addr = "0xdac17f958d2ee523a2206206994597c13d831ec7";
        // transfer(address,uint256) selector
        let sel = [0xa9, 0x05, 0x9c, 0xbb];

        let func = fetch_etherscan_abi(addr, sel, &api_key, None)
            .await
            .unwrap();
        assert_eq!(func.name, "transfer");
        assert_eq!(func.inputs.len(), 2);
    }
}
