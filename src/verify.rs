use eyre::{Result, bail};
use tracing::{debug, info, trace};

use crate::project::CompiledRuntimeBytecode;

/// Compare compiled deployed bytecode against on-chain bytecode.
///
/// Byte ranges listed in `compiled.placeholders` (library addresses,
/// immutable values, library self-address) are excluded from comparison.
/// CBOR metadata appended by the compiler is stripped from both sides
/// before comparison.
pub fn verify_deployed_bytecode(compiled: &CompiledRuntimeBytecode, onchain: &[u8]) -> Result<()> {
    let placeholders = &compiled.placeholders;

    info!(
        compiled_len = compiled.bytes.len(),
        onchain_len = onchain.len(),
        placeholder_regions = placeholders.len(),
        "Starting bytecode comparison"
    );

    // Strip CBOR metadata from both sides.  The last two bytes of solc
    // output encode the metadata section length as a big-endian u16.
    let compiled_code_stripped = strip_cbor_metadata(&compiled.bytes);
    let onchain_code_stripped = strip_cbor_metadata(onchain);

    debug!(
        compiled_metadata_bytes = compiled.bytes.len() - compiled_code_stripped.len(),
        onchain_metadata_bytes = onchain.len() - onchain_code_stripped.len(),
        "Stripped CBOR metadata"
    );

    let mut compiled_code = compiled_code_stripped.to_vec();
    let mut onchain_code = onchain_code_stripped.to_vec();

    debug!(
        compiled_len = compiled_code.len(),
        onchain_len = onchain_code.len(),
        "Code lengths after stripping metadata"
    );
    trace!(compiled_hex = %alloy::hex::encode(&compiled_code), "Compiled bytecode after metadata strip");
    trace!(onchain_hex = %alloy::hex::encode(&onchain_code), "On-chain bytecode after metadata strip");

    if compiled_code.len() != onchain_code.len() {
        bail!(
            "code length mismatch after stripping metadata: compiled {} bytes vs on-chain {} bytes",
            compiled_code.len(),
            onchain_code.len(),
        );
    }

    // Zero out placeholder regions in both buffers so they compare equal.
    for r in placeholders {
        let end = r.end.min(compiled_code.len());
        let start = r.start.min(end);
        debug!(start, end, "Zeroing out placeholder region");
        compiled_code[start..end].fill(0);
        onchain_code[start..end].fill(0);
    }

    if compiled_code == onchain_code {
        info!("Bytecode verification succeeded");
        Ok(())
    } else {
        let mismatches: Vec<usize> = compiled_code
            .iter()
            .zip(onchain_code.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        let preview: Vec<String> = mismatches
            .iter()
            .take(10)
            .map(|&i| {
                format!(
                    "  offset {i}: compiled 0x{:02x}, on-chain 0x{:02x}",
                    compiled_code[i], onchain_code[i]
                )
            })
            .collect();
        let suffix = if mismatches.len() > 10 {
            format!("\n  ... and {} more", mismatches.len() - 10)
        } else {
            String::new()
        };
        bail!(
            "bytecode mismatch at {} byte(s):\n{}{}",
            mismatches.len(),
            preview.join("\n"),
            suffix,
        )
    }
}

/// Strip the CBOR-encoded metadata appended by solc.
/// The last two bytes are a big-endian u16 giving the metadata length.
fn strip_cbor_metadata(bytecode: &[u8]) -> &[u8] {
    if bytecode.len() < 2 {
        return bytecode;
    }
    let metadata_len =
        u16::from_be_bytes([bytecode[bytecode.len() - 2], bytecode[bytecode.len() - 1]]) as usize;
    let total_suffix = metadata_len + 2;
    if total_suffix >= bytecode.len() {
        return bytecode;
    }
    &bytecode[..bytecode.len() - total_suffix]
}
