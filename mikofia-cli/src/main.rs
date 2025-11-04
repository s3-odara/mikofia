use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

mod commands;
mod templates;

#[derive(Parser, Debug)]
#[command(name = "mikofia")]
#[command(version, about = "A file structure validation tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to the configuration file (supports .json, .js, and .ts)
    /// Only used when no subcommand is specified (defaults to 'check')
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Directory to check (defaults to current directory)
    /// Only used when no subcommand is specified (defaults to 'check')
    #[arg(short, long, global = true)]
    dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check the file structure against the configuration
    Check {
        /// Path to the configuration file (supports .json, .js, and .ts)
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Directory to check (defaults to current directory)
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
    /// Initialize a new mikofia configuration file
    Init {
        /// Configuration format (ts, js, or json)
        #[arg(short, long, default_value = "ts")]
        format: templates::ConfigFormat,

        /// Custom config file path
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Directory to create config in (defaults to current directory)
        #[arg(short, long)]
        dir: Option<PathBuf>,

        /// Overwrite existing config file without prompting
        #[arg(long)]
        force: bool,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Some(Commands::Check { config, dir }) => commands::check::run(config, dir).await,
        Some(Commands::Init {
            format,
            config,
            dir,
            force,
        }) => commands::init::run(format, config, dir, force).await,
        None => {
            // No subcommand specified, default to 'check' with global flags
            commands::check::run(cli.config, cli.dir).await
        }
    };

    process::exit(exit_code);
}
