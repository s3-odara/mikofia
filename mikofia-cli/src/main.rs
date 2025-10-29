use clap::Parser;
use mikofia::Reporter;
use std::path::PathBuf;
use std::process;

#[derive(Parser, Debug)]
#[command(name = "mikofia")]
#[command(version, about = "A file structure validation tool", long_about = None)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = "mikofia.config.json")]
    config: PathBuf,

    /// Directory to check (defaults to current directory)
    #[arg(short, long)]
    dir: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    let current_dir = std::env::current_dir().unwrap();

    // Determine the directory to check
    let check_dir = args.dir.unwrap_or_else(|| current_dir.clone());

    let config_path = if args.config.is_absolute() {
        args.config
    } else {
        check_dir.join(&args.config)
    };

    if !config_path.exists() {
        eprintln!("❌ Config file not found: {}", config_path.display());
        eprintln!("   Create a config file or specify a different path with --config");
        process::exit(2);
    }

    // Load config file
    let config = match mikofia::Config::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Failed to load config: {}", e);
            process::exit(2);
        }
    };

    println!("🚀 Running mikofia check...\n");
    println!("📁 Directory: {}", check_dir.display());
    println!("⚙️  Config: {}\n", config_path.display());

    // Run validation
    let violations = mikofia::check(&config.nodes, &check_dir);

    // Convert violations to results and report
    let results = mikofia::violations_to_results(&violations);
    let reporter = mikofia::ConsoleReporter;
    reporter.report(&results);

    // Exit with appropriate code
    if violations.is_empty() {
        process::exit(0);
    } else {
        process::exit(1);
    }
}
