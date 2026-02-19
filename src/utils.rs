use std::path::Path;

use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use eyre::{WrapErr, bail, eyre};
use tracing::{debug, info, trace};

/// Parse a hex string (with optional `0x` prefix) into bytes.
pub fn parse_bytecode_hex(hex_str: &str) -> eyre::Result<Vec<u8>> {
    let hex = hex_str.trim().strip_prefix("0x").unwrap_or(hex_str.trim());
    alloy::hex::decode(hex).map_err(|e| eyre!("invalid hex: {e}"))
}

/// Read a bytecode file. Accepts hex with optional `0x` prefix.
pub fn read_bytecode_file(path: &Path) -> eyre::Result<Vec<u8>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| eyre!("failed to read {}: {e}", path.display()))?;
    parse_bytecode_hex(&text)
}

/// Resolve a value into on-chain bytecode bytes.
///
/// Heuristic auto-detect:
/// - 42-char `0x`-prefixed hex → Ethereum address (fetched via RPC)
/// - Existing file on disk → read hex from file
/// - Otherwise → treat as inline hex bytecode
pub async fn resolve_bytecode(value: &str, rpc_url: &str) -> eyre::Result<Vec<u8>> {
    let trimmed = value.trim();

    // 1. Check if it's a 20-byte Ethereum address (0x + 40 hex chars)
    if trimmed.len() == 42
        && trimmed.starts_with("0x")
        && trimmed[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        info!(address = trimmed, rpc_url, "Fetching bytecode from chain");
        let bytes = fetch_bytecode_rpc(trimmed, rpc_url).await?;
        debug!(size = bytes.len(), "Fetched bytecode from RPC");
        trace!(bytes = %alloy::hex::encode(&bytes), "Resolved bytecode prefix");
        return Ok(bytes);
    }

    // 2. Check if it's a file path
    let path = Path::new(trimmed);
    if path.exists() {
        info!(path = %path.display(), "Reading bytecode from file");
        let bytes = read_bytecode_file(path)?;
        debug!(size = bytes.len(), "Read bytecode from file");
        trace!(bytes = %alloy::hex::encode(&bytes), "Resolved bytecode prefix");
        return Ok(bytes);
    }

    // 3. Treat as inline hex
    info!("Using inline hex bytecode");
    let bytes = parse_bytecode_hex(trimmed)?;
    debug!(size = bytes.len(), "Parsed inline hex bytecode");
    trace!(bytes = %alloy::hex::encode(&bytes), "Resolved bytecode prefix");
    Ok(bytes)
}

/// Fetch deployed bytecode for an address via `eth_getCode`.
async fn fetch_bytecode_rpc(address: &str, rpc_url: &str) -> eyre::Result<Vec<u8>> {
    let addr: Address = address.parse().wrap_err("invalid address")?;
    let url = rpc_url.parse().wrap_err("invalid RPC URL")?;
    let provider = ProviderBuilder::new().connect_http(url);

    let code = provider
        .get_code_at(addr)
        .await
        .wrap_err_with(|| format!("failed to fetch bytecode for {address} from {rpc_url}"))?;

    if code.is_empty() {
        bail!("no bytecode at address {address} (EOA or self-destructed contract)");
    }

    Ok(code.to_vec())
}
