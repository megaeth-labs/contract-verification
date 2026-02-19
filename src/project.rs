use std::ops::Range;
use std::path::{Path, PathBuf};

use eyre::{Result, WrapErr, bail, eyre};
use foundry_compilers::compilers::solc::Solc;
use foundry_compilers_artifacts_solc::{CompilerOutput, SolcInput};
use tracing::{debug, info, trace, warn};

/// Solc specified as either a filesystem path or a version string.
#[derive(Clone, Debug)]
pub enum SolcRef {
    Path(PathBuf),
    Version(semver::Version),
}

impl std::str::FromStr for SolcRef {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Ok(v) = s.parse::<semver::Version>() {
            Ok(SolcRef::Version(v))
        } else {
            Ok(SolcRef::Path(PathBuf::from(s)))
        }
    }
}

pub enum Project {
    StandardJson { input: SolcInput, solc: Solc },
}

/// Compiled runtime bytecode together with the byte ranges that are
/// expected to differ between the compiler output and the on-chain copy
/// (library addresses, immutable values, library self-address).
pub struct CompiledRuntimeBytecode {
    pub bytes: Vec<u8>,
    pub placeholders: Vec<Range<usize>>,
}

impl Project {
    pub fn standard_json(input_path: &Path, solc: Option<&SolcRef>) -> Result<Self> {
        debug!(path = %input_path.display(), "Reading standard JSON input");
        let contents = std::fs::read_to_string(input_path)
            .wrap_err_with(|| format!("failed to read {}", input_path.display()))?;
        let input: SolcInput = serde_json::from_str(&contents)
            .wrap_err_with(|| format!("failed to parse {} as SolcInput", input_path.display()))?;
        let solc = match solc {
            Some(SolcRef::Version(v)) => {
                info!(version = %v, "Using solc version");
                Solc::find_or_install(v)
                    .wrap_err_with(|| format!("failed to find or install solc {v}"))?
            }
            Some(SolcRef::Path(p)) => {
                info!(path = %p.display(), "Using solc at path");
                Solc::new(p).wrap_err_with(|| format!("invalid solc path: {}", p.display()))?
            }
            None => {
                info!("Using solc from PATH");
                Solc::new("solc").wrap_err("failed to find solc on PATH")?
            }
        };

        Ok(Project::StandardJson { input, solc })
    }

    pub fn compiled_runtime_bytecode(
        &self,
        contract_name: &str,
    ) -> Result<CompiledRuntimeBytecode> {
        match self {
            Project::StandardJson { input, solc } => {
                debug!("Compilation starting");
                let compiler_output: CompilerOutput =
                    solc.compile(input).wrap_err("solc compilation failed")?;
                debug!("Compilation completed");

                // Log compiler warnings
                for diag in &compiler_output.errors {
                    if !diag.severity.is_error() {
                        if let Some(msg) = &diag.formatted_message {
                            warn!("Solc warning: {msg}");
                        }
                    }
                }

                // Check for compilation errors
                let errors: Vec<_> = compiler_output
                    .errors
                    .iter()
                    .filter(|e| e.severity.is_error())
                    .collect();
                if !errors.is_empty() {
                    let msgs: Vec<String> = errors
                        .iter()
                        .map(|e| e.formatted_message.clone().unwrap_or_default())
                        .collect();
                    bail!("solc compilation errors:\n{}", msgs.join("\n"));
                }

                // Search for the contract across all files
                for (source_file, contracts) in &compiler_output.contracts {
                    if let Some(contract) = contracts.get(contract_name) {
                        info!(contract = contract_name, source = %source_file.display(), "Found contract");
                        let evm = contract
                            .evm
                            .as_ref()
                            .ok_or_else(|| eyre!("no evm output for contract {contract_name}"))?;
                        let deployed = evm.deployed_bytecode.as_ref().ok_or_else(|| {
                            eyre!("no deployed bytecode for contract {contract_name}")
                        })?;

                        let bytecode = deployed.bytecode.as_ref().ok_or_else(|| {
                            eyre!("no bytecode object for contract {contract_name}")
                        })?;

                        let bytes = bytecode
                            .object
                            .as_bytes()
                            .ok_or_else(|| {
                                eyre!("bytecode is unlinked — libraries must be linked first")
                            })?
                            .to_vec();

                        debug!(size = bytes.len(), "Compiled bytecode size");
                        trace!(hex = %alloy::hex::encode(&bytes), "Raw compiled bytecode");

                        let mut placeholders = Vec::new();

                        // link_references: library address slots
                        for libs in bytecode.link_references.values() {
                            for offsets in libs.values() {
                                for o in offsets {
                                    let start = o.start as usize;
                                    let end = start + o.length as usize;
                                    debug!(start, end, "Link reference placeholder");
                                    placeholders.push(start..end);
                                }
                            }
                        }

                        // immutable_references: constructor-set immutable values
                        for offsets in deployed.immutable_references.values() {
                            for o in offsets {
                                let start = o.start as usize;
                                let end = start + o.length as usize;
                                debug!(start, end, "Immutable reference placeholder");
                                placeholders.push(start..end);
                            }
                        }

                        // Library self-address: PUSH20 (0x73) followed by 20 zero
                        // bytes.  solc emits this for a library's own address and
                        // does NOT record it in linkReferences.
                        detect_push_zero_placeholders(&bytes, &mut placeholders);

                        placeholders.sort_by_key(|r| r.start);
                        info!(count = placeholders.len(), "Total placeholders collected");

                        return Ok(CompiledRuntimeBytecode {
                            bytes,
                            placeholders,
                        });
                    }
                }

                bail!("contract {contract_name} not found in compilation output")
            }
        }
    }
}

/// Detect PUSH20 (0x73) followed by 20 zero bytes at the start of the
/// compiled bytecode — the pattern solc uses for a library's own address
/// guard.  Only checks byte 0 since solc always emits this as the first
/// instruction of library runtime bytecode.
fn detect_push_zero_placeholders(compiled: &[u8], placeholders: &mut Vec<Range<usize>>) {
    const PUSH20: u8 = 0x73;
    const ADDR_LEN: usize = 20;

    if compiled.first() == Some(&PUSH20)
        && compiled.len() > ADDR_LEN
        && compiled[1..1 + ADDR_LEN].iter().all(|&b| b == 0)
    {
        debug!(
            start = 1,
            end = 1 + ADDR_LEN,
            "Detected library self-address placeholder"
        );
        placeholders.push(1..1 + ADDR_LEN);
    }
}
