use alloy::{
    consensus::Transaction,
    hex,
    primitives::{Bytes, TxKind},
    providers::{Provider, ProviderBuilder},
};
use clap::Parser;
use eyre::eyre;
use reqwest::Url;

// Local modules
mod cache;
mod decode;
mod display;
mod etherscan;
mod signatures;

#[derive(Parser, Debug)]
#[command(
    about = "🔍 Decode EVM transaction calldata",
    long_about = "🔍 A custom Ethereum transaction decoder built with Alloy.\n\
                  Supports 4byte.directory lookups, Etherscan ABI fallback, and local caching.",
    version,
    author = "s3bc40 <s3bc40@gmail.com>"
)]
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

async fn fetch_and_decode_tx(
    tx_hash: &str,
    rpc_url: &str,
    etherscan_key: Option<&str>,
) -> eyre::Result<()> {
    let url = Url::parse(rpc_url)?;
    let provider = ProviderBuilder::new().connect_http(url);

    // Fetch the transaction by hash
    let tx_hash = tx_hash.parse()?;
    let tx = provider
        .get_transaction_by_hash(tx_hash)
        .await?
        .ok_or_else(|| eyre!("transaction not found"))?;

    // Access the inner transaction through the public `inner` field
    // The `inner` is a Recovered<T> which wraps the actual transaction
    let inner_tx = tx.inner.inner();

    // Now use the TransactionTrait methods on the inner transaction
    let calldata = inner_tx.input();
    let value = inner_tx.value();
    let kind = inner_tx.kind();

    // Get sender from the Recovered wrapper
    let from = tx.inner.signer();

    // Extract address from TxKind
    let to = match kind {
        TxKind::Call(addr) => Some(addr),
        TxKind::Create => None,
    };

    println!("📡 Fetched transaction: 0x{}", hex::encode(tx_hash));
    println!("   From: {:?}", from);
    println!("   To: {:?}", to.unwrap_or_default());
    println!("   Value: {} wei\n", value);

    if calldata.is_empty() {
        println!("ℹ️ No calldata to decode (empty input).");
        return Ok(());
    }

    // Decode the calldata
    let contract_address = to.map(|addr| format!("{:?}", addr));
    match decode::decode_calldata(calldata, contract_address.as_deref(), etherscan_key).await {
        Ok((func, params)) => display::display_decoded(&func.name, &params, &func),
        Err(e) => println!("❌ failed to decode calldata after fetch: {}", e),
    }

    Ok(())
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

        match decode::decode_calldata(&bytes, None, args.etherscan_key.as_deref()).await {
            Ok((func, params)) => display::display_decoded(&func.name, &params, &func),
            Err(e) => println!("❌ failed to decode calldata: {}", e),
        }
        return Ok(());
    }

    // Fetch transaction by hash (Step 10)
    if let Some(tx_hash) = args.tx_hash {
        fetch_and_decode_tx(&tx_hash, &args.rpc, args.etherscan_key.as_deref()).await?;
        return Ok(());
    }

    eprintln!("❌ error: Provide either a transaction hash or --input <calldata>");
    Ok(())
}
