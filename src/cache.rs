use std::{env, fs, path::PathBuf};

use alloy_json_abi::Function;

/// Returns the path to the cache directory, creating it if it doesn't exist.
pub fn cache_dir() -> eyre::Result<PathBuf> {
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE"))?;
    let cache = PathBuf::from(home).join(".txdecode").join("cache");
    fs::create_dir_all(&cache)?;
    Ok(cache)
}

/// Returns the cache file path for a given contract address.
pub fn cache_path(address: &str) -> eyre::Result<PathBuf> {
    Ok(cache_dir()?.join(format!("{}.json", address.to_lowercase())))
}

/// Loads the cached ABI for the given contract address, if it exists.
pub fn load_cache_abi(address: &str) -> Option<Vec<Function>> {
    let path = cache_path(address).ok()?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Saves the given ABI to the cache for the specified contract address.
pub fn save_cached_abi(address: &str, abi: &[Function]) -> eyre::Result<()> {
    let path = cache_path(address)?;
    let json = serde_json::to_string_pretty(abi)?;
    fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_dir_creation() {
        let dir = cache_dir().unwrap();
        assert!(dir.exists());
        assert!(dir.ends_with(".txdecode/cache"));
    }

    #[test]
    fn test_cache_path_normalization() {
        let address = "0xAbC123";
        let path = cache_path(address).unwrap();
        assert!(path.ends_with("0xabc123.json"));
    }
}
