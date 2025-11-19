# 🔍 txdecode

A **blazingly fast** EVM transaction decoder CLI built in Rust, powered exclusively by [Alloy](https://github.com/alloy-rs/alloy).

Decode any Ethereum transaction or raw calldata into human-readable function calls and parameters — no more squinting at hex blobs.

---

## ✨ Features

- 🚀 **Automatic function signature detection** via [4byte.directory](https://www.4byte.directory/)
- 🎯 **Smart collision handling** — prioritizes well-known ERC-20/ERC-721 functions
- 🎨 **Beautiful terminal output** with color-coded tables (coming soon)
- ⚡ **Pure Alloy** — no legacy dependencies (ethers-rs, web3, etc.)
- 🔒 **Type-safe ABI decoding** with comprehensive error handling

---

## 🚧 Current Status

**Working:**

- ✅ Extract 4-byte function selectors
- ✅ Query 4byte.directory API
- ✅ Parse Solidity signatures dynamically
- ✅ Decode calldata with prioritized signature matching
- ✅ Handle hash collisions (scam/honeypot filters)

**Coming Soon:**

- 🔜 Fetch transactions from RPC providers
- 🔜 Etherscan/Sourcify verified ABI fallback
- 🔜 Local ABI cache
- 🔜 ENS reverse lookup for addresses
- 🔜 Token symbol/decimal enrichment
- 🔜 Multi-chain support (Base, Arbitrum, Optimism, etc.)
- 🔜 Decode internal calls via `trace_transaction`

---

## 📦 Installation

```bash
git clone https://github.com/yourusername/txdecode.git
cd txdecode
cargo build --release
```

---

## 🎯 Usage

### Decode a transaction by hash (coming soon)

```bash
txdecode 0x1234...abcd
```

### Decode raw calldata (current)

```bash
txdecode --input 0xa9059cbb0000000000000000000000000742d35cc...
```

### Specify RPC endpoint

```bash
txdecode --rpc https://eth.llamarpc.com 0x1234...abcd
```

### Use chain presets (coming soon)

```bash
txdecode --chain base 0x1234...abcd
```

---

## 🛠️ Tech Stack

| Component                 | Library                              |
| ------------------------- | ------------------------------------ |
| **Ethereum types**        | `alloy::primitives`                  |
| **ABI encoding/decoding** | `alloy::sol_types`, `alloy_json_abi` |
| **RPC provider**          | `alloy::providers`                   |
| **HTTP client**           | `reqwest`                            |
| **Error handling**        | `color-eyre`                         |
| **CLI parsing**           | `clap`                               |
| **Pretty tables**         | `comfy-table`                        |

---

## 🧪 Example Output

```
Decoding calldata (68 bytes)...

✅ Decoded using function: transfer

Parameters:
  [0]: Address(0x0742d35cc6634c0532925a3b844bc9e7595f0beb)
  [1]: Uint(1000000, 256)
```

---

## 🗺️ Roadmap

1. ✅ **Step 1-4:** Selector extraction + 4byte lookup + signature parsing + decoding
2. 🔜 **Step 5:** Etherscan/Sourcify verified ABI fallback
3. 🔜 **Step 6:** Local file cache for fetched ABIs
4. 🔜 **Step 7:** Value enrichment (ENS, token metadata, formatting)
5. 🔜 **Step 8:** Gorgeous `comfy-table` output
6. 🔜 **Step 9:** Full raw calldata input support
7. 🔜 **Step 10:** Internal call tracing

---

## 📄 License

MIT

---

## 🙏 Acknowledgments

- [Alloy](https://github.com/alloy-rs/alloy) — Modern Ethereum library
- [4byte.directory](https://www.4byte.directory/) — Function signature database
