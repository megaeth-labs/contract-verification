use tracing::Level;
use tracing_subscriber::EnvFilter;

#[derive(clap::Args, Debug)]
pub struct LogArgs {
    /// Verbosity level (repeat for more: -v, -vv, -vvv, -vvvv, -vvvvv)
    #[arg(short = 'v', long = "verbosity", action = clap::ArgAction::Count)]
    pub verbosity: u8,

    /// Disable colored log output
    #[arg(long = "log.no-color")]
    pub log_no_color: bool,
}

impl LogArgs {
    pub fn init(&self) {
        let filter = if std::env::var("RUST_LOG").is_ok() {
            // Use RUST_LOG if set
            EnvFilter::from_default_env()
        } else if self.verbosity == 0 {
            // No verbosity: no logs
            EnvFilter::new("off")
        } else {
            // Verbosity-based level
            let level = match self.verbosity {
                1 => Level::ERROR,
                2 => Level::WARN,
                3 => Level::INFO,
                4 => Level::DEBUG,
                _ => Level::TRACE,
            };
            EnvFilter::new(format!("contract_verification={level}"))
        };

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(!self.log_no_color)
            .init();
    }
}
