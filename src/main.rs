mod logging;
mod project;
mod utils;
mod verify;

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "verify-contract",
    about = "Verify smart contract bytecode against source code"
)]
struct Cli {
    /// Path to solc standard JSON input file
    #[arg(short = 'i', long = "standard-json-input")]
    standard_json_input: PathBuf,

    /// Contract name to verify from compilation output
    #[arg(short = 'c', long)]
    contract_name: String,

    /// Solc version or path (e.g. 0.8.28 or /usr/bin/solc).
    /// When omitted, uses `solc` from PATH.
    #[arg(short = 's', long)]
    solc: Option<project::SolcRef>,

    /// RPC endpoint URL (required when address is used)
    #[arg(
        short = 'r',
        long,
        default_value = "http://localhost:8545",
        env = "RPC_URL"
    )]
    rpc_url: String,

    /// Contract address, path to bytecode file, or inline hex bytecode.
    /// Auto-detected: 0x + 40 hex chars → address; existing file → read from
    /// file; otherwise → inline hex.
    address_or_bytecode: String,

    #[command(flatten)]
    log: logging::LogArgs,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    cli.log.init();

    if let Err(e) = run(&cli).await {
        eprintln!("error: {e:?}");
        std::process::exit(1);
    }
}

async fn run(cli: &Cli) -> eyre::Result<()> {
    // Read on-chain bytecode
    let onchain_bytes = utils::resolve_bytecode(&cli.address_or_bytecode, &cli.rpc_url).await?;

    // Compile source and extract deployed bytecode + placeholder ranges
    let project =
        project::Project::standard_json(&cli.standard_json_input, cli.solc.as_ref())?;
    let compiled = project.compiled_runtime_bytecode(&cli.contract_name)?;

    // Compare, skipping placeholder regions
    verify::verify_deployed_bytecode(&compiled, &onchain_bytes)?;

    println!("verification succeeded");
    Ok(())
}
