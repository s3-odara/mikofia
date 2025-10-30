use clap::Parser;
use std::path::PathBuf;
use std::process;

use mikofia::Reporter;

#[derive(Parser, Debug)]
#[command(name = "mikofia")]
#[command(version, about = "A file structure validation tool", long_about = None)]
struct Args {
    /// Path to the configuration file (supports .json and .js)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Directory to check (defaults to current directory)
    #[arg(short, long)]
    dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let current_dir = std::env::current_dir().unwrap();

    // Determine the directory to check
    let check_dir = args.dir.unwrap_or_else(|| current_dir.clone());

    // Find config file: check both .js and .json if not specified
    let config_path = match args.config {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                check_dir.join(&path)
            }
        }
        None => {
            // Try mikofia.config.js first, then mikofia.config.json
            let js_path = check_dir.join("mikofia.config.js");
            let json_path = check_dir.join("mikofia.config.json");

            if js_path.exists() {
                js_path
            } else if json_path.exists() {
                json_path
            } else {
                eprintln!("❌ Config file not found");
                eprintln!("   Looked for:");
                eprintln!("   - {}", js_path.display());
                eprintln!("   - {}", json_path.display());
                eprintln!("\n   Create a config file or specify a path with --config");
                process::exit(2);
            }
        }
    };

    if !config_path.exists() {
        eprintln!("❌ Config file not found: {}", config_path.display());
        eprintln!("   Create a config file or specify a different path with --config");
        process::exit(2);
    }

    // Load config file based on extension
    let config = match load_config(&config_path).await {
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

/// Load configuration from either JSON or JavaScript file
async fn load_config(
    path: &PathBuf,
) -> Result<mikofia::Config, Box<dyn std::error::Error + Send + Sync>> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("js") => {
            // Load JavaScript config using Deno runtime
            mikofia_deno::load_javascript_config(path).await
        }
        Some("json") | None => {
            // Load JSON config using existing method
            mikofia::Config::from_file(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
        Some(ext) => Err(format!("Unsupported config file extension: .{}", ext).into()),
    }
}
