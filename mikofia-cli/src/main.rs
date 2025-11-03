use clap::Parser;
use std::path::PathBuf;
use std::process;

use mikofia::Reporter;

/// Configuration file type based on extension
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigType {
    /// Deno-based config (.js, .ts)
    Deno,
    /// JSON config (.json)
    Json,
}

/// Pure function to determine config type from file extension
fn config_type_from_extension(ext: &str) -> Option<ConfigType> {
    match ext {
        "js" | "ts" => Some(ConfigType::Deno),
        "json" => Some(ConfigType::Json),
        _ => None,
    }
}

#[derive(Parser, Debug)]
#[command(name = "mikofia")]
#[command(version, about = "A file structure validation tool", long_about = None)]
struct Args {
    /// Path to the configuration file (supports .json, .js, and .ts)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Directory to check (defaults to current directory)
    #[arg(short, long)]
    dir: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();

    let current_dir = std::env::current_dir().unwrap();

    // Determine the directory to check
    let check_dir = args.dir.unwrap_or_else(|| current_dir.clone());

    // Find config file: check .ts, .js, and .json if not specified
    let config_path = match args.config {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                check_dir.join(&path)
            }
        }
        None => {
            // Try in order: .ts → .js → .json (TypeScript preferred)
            let candidates = ["mikofia.config.ts", "mikofia.config.js", "mikofia.config.json"];

            let found = candidates
                .iter()
                .map(|name| check_dir.join(name))
                .find(|path| path.exists());

            match found {
                Some(path) => path,
                None => {
                    eprintln!("❌ Config file not found");
                    eprintln!("   Looked for:");
                    for candidate in &candidates {
                        eprintln!("   - {}", check_dir.join(candidate).display());
                    }
                    eprintln!("\n   Create a config file or specify a path with --config");
                    process::exit(2);
                }
            }
        }
    };

    if !config_path.exists() {
        eprintln!("❌ Config file not found: {}", config_path.display());
        eprintln!("   Create a config file or specify a different path with --config");
        process::exit(2);
    }

    // Load config file and run checks based on extension
    println!("🚀 Running mikofia check...\n");
    println!("📁 Directory: {}", check_dir.display());
    println!("⚙️  Config: {}\n", config_path.display());

    let config_type = config_path
        .extension()
        .and_then(|s| s.to_str())
        .and_then(config_type_from_extension);

    let violations = match config_type {
        Some(ConfigType::Deno) => {
            // Load Deno config (JavaScript/TypeScript) with custom rules
            let mut runtime = match mikofia_deno::DenoRuntime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("❌ Failed to initialize Deno runtime: {}", e);
                    process::exit(2);
                }
            };

            let (config, rules_map) = match runtime.load_config_with_rules(&config_path).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("❌ Failed to load config: {}", e);
                    process::exit(2);
                }
            };

            // Run checks with JavaScript rules
            match mikofia_deno::check_with_javascript_rules(
                config,
                &check_dir,
                rules_map,
                &mut runtime,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ Failed to run checks: {}", e);
                    process::exit(2);
                }
            }
        }
        Some(ConfigType::Json) | None => {
            // Load JSON config
            let config = match load_config(&config_path).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("❌ Failed to load config: {}", e);
                    process::exit(2);
                }
            };

            // Create ignore matcher from config
            let ignore_matcher = match mikofia::IgnoreMatcher::new(&config.ignore) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("❌ Failed to create ignore matcher: {}", e);
                    process::exit(2);
                }
            };

            // Run standard checks
            mikofia::check_with_ignore(&config.nodes, &check_dir, &ignore_matcher)
        }
    };

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

/// Load configuration from JSON or Deno-based file (JavaScript/TypeScript)
async fn load_config(
    path: &PathBuf,
) -> Result<mikofia::Config, Box<dyn std::error::Error + Send + Sync>> {
    let config_type = path
        .extension()
        .and_then(|s| s.to_str())
        .and_then(config_type_from_extension);

    match config_type {
        Some(ConfigType::Deno) => {
            // Load Deno config (JavaScript/TypeScript) using Deno runtime
            mikofia_deno::load_deno_config(path).await
        }
        Some(ConfigType::Json) | None => {
            // Load JSON config using existing method
            mikofia::Config::from_file(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }
}
