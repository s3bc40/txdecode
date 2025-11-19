use alloy::primitives::Bytes;
use eyre::eyre;

/// Extracts the first four bytes from the given byte slice to use as a function selector.
fn selector(data: &Bytes) -> eyre::Result<[u8; 4]> {
    data.get(..4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| eyre!("data too short to extract selector"))
}

fn main() {
    println!("Hello, world!");
}
