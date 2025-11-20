use alloy::{
    dyn_abi::{DynSolValue, JsonAbiExt},
    hex,
    primitives::Bytes,
};
use alloy_json_abi::Function;
use eyre::{bail, eyre};

use crate::etherscan;
use crate::signatures;

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
pub async fn decode_calldata(
    calldata: &Bytes,
    contract_address: Option<&str>,
    etherscan_key: Option<&str>,
    chain: Option<u64>,
) -> eyre::Result<(Function, Vec<DynSolValue>)> {
    let sel = signatures::selector(calldata)?;
    let signatures = signatures::lookup_selector(sel).await?;
    let chain_id = chain.unwrap_or(1);

    if signatures.is_empty() {
        bail!("no signatures found for selector 0x{}", hex::encode(sel));
    }

    // Priroritize common signatures (e.g., ERC-20 transfer)
    let mut prioritized: Vec<&String> = signatures
        .iter()
        .filter(|s| {
            signatures::WELL_KNOWN_FUNC_NAME
                .iter()
                .any(|wk| s.starts_with(wk))
        })
        .collect();

    // Append the rest of the signatures
    prioritized.extend(signatures.iter().filter(|s| {
        !signatures::WELL_KNOWN_FUNC_NAME
            .iter()
            .any(|wk| s.starts_with(wk))
    }));

    // Try to decode using each signature until one works
    for sig in prioritized {
        if let Ok(func) = signatures::parse_signature(sig) {
            if let Ok(decoded) = try_decode(&func, calldata) {
                return Ok((func, decoded));
            }
        }
    }

    // Fallback to Etherscan ABI if contract address and API key are provided
    if let (Some(addr), Some(key)) = (contract_address, etherscan_key) {
        let func = etherscan::fetch_etherscan_abi(chain_id, addr, sel, key).await?;
        let decoded = try_decode(&func, calldata)?;
        return Ok((func, decoded));
    }

    bail!(
        "all {} signatures failed to decode calldata",
        signatures.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_decode_transfer() {
        let sig = "transfer(address,uint256)";
        let func = signatures::parse_signature(sig).unwrap();

        let calldata = Bytes::from(
            hex::decode("a9059cbb0000000000000000000000000742d35cc6634c0532925a3b844bc9e7595f0beb00000000000000000000000000000000000000000000000000000000000f4240").unwrap()
        );

        let decoded = try_decode(&func, &calldata).unwrap();
        assert_eq!(decoded.len(), 2);
    }

    #[tokio::test]
    async fn test_decode_calldata() {
        let calldata = Bytes::from(
            hex::decode("a9059cbb0000000000000000000000000742d35cc6634c0532925a3b844bc9e7595f0beb00000000000000000000000000000000000000000000000000000000000f4240").unwrap()
        );

        let (func, params) = decode_calldata(&calldata, None, None, None).await.unwrap();
        assert_eq!(func.name, "transfer");
        assert_eq!(params.len(), 2);
    }
}
