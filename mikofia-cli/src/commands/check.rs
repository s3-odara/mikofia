use std::path::{Path, PathBuf};

use mikofia::Reporter;

/// Configuration file type based on extension
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigType {
    /// Deno-based config (.js, .ts)
    Deno,
    /// JSON config (.json)
    Json,
}

/// Pure function to determine config type from file extension
pub fn config_type_from_extension(ext: &str) -> Option<ConfigType> {
    let is_deno = ["js", "ts"]
        .iter()
        .any(|candidate| ext.eq_ignore_ascii_case(candidate));

    if is_deno {
        Some(ConfigType::Deno)
    } else if ext.eq_ignore_ascii_case("json") {
        Some(ConfigType::Json)
    } else {
        None
    }
}

/// Pure function to determine the directory to check
pub fn determine_check_directory(dir: Option<PathBuf>) -> Result<PathBuf, std::io::Error> {
    match dir {
        Some(d) => Ok(d),
        None => std::env::current_dir(),
    }
}

/// Pure function to find config file
pub fn find_config_file(
    dir: &Path,
    explicit: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match explicit {
        Some(path) => {
            let resolved = if path.is_absolute() {
                path
            } else {
                dir.join(&path)
            };
            Ok(resolved)
        }
        None => {
            // Try in order: .ts → .js → .json (TypeScript preferred)
            let candidates = [
                "mikofia.config.ts",
                "mikofia.config.js",
                "mikofia.config.json",
            ];

            candidates
                .iter()
                .map(|name| dir.join(name))
                .find(|path| path.exists())
                .ok_or_else(|| {
                    format!(
                        "Config file not found. Looked for: {}",
                        candidates
                            .iter()
                            .map(|c| dir.join(c).display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .into()
                })
        }
    }
}

/// Load configuration from JSON or Deno-based file (JavaScript/TypeScript)
async fn load_config(
    path: &Path,
) -> Result<mikofia::Config, Box<dyn std::error::Error + Send + Sync>> {
    let raw_extension = path.extension().and_then(|s| s.to_str());

    match raw_extension {
        Some(ext) => match config_type_from_extension(ext) {
            Some(ConfigType::Deno) => {
                // Load Deno config (JavaScript/TypeScript) using Deno runtime
                mikofia_deno::load_deno_config(path).await
            }
            Some(ConfigType::Json) => {
                // Load JSON config using existing method
                mikofia::Config::from_file(path)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            None => Err(format!("Unsupported config file extension: .{}", ext).into()),
        },
        None => mikofia::Config::from_file(path)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
    }
}

/// Run the check command
pub async fn run(config: Option<PathBuf>, dir: Option<PathBuf>) -> i32 {
    // Determine the directory to check
    let check_dir = match determine_check_directory(dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("❌ Failed to determine check directory: {}", e);
            return 2;
        }
    };

    // Find config file
    let config_path = match find_config_file(&check_dir, config) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("❌ {}", e);
            eprintln!("   Create a config file or specify a path with --config");
            return 2;
        }
    };

    if !config_path.exists() {
        eprintln!("❌ Config file not found: {}", config_path.display());
        eprintln!("   Create a config file or specify a different path with --config");
        return 2;
    }

    // Load config file and run checks based on extension
    println!("🚀 Running mikofia check...\n");
    println!("📁 Directory: {}", check_dir.display());
    println!("⚙️  Config: {}\n", config_path.display());

    let raw_extension = config_path.extension().and_then(|s| s.to_str());
    let config_type = raw_extension.and_then(config_type_from_extension);

    if raw_extension.is_some() && config_type.is_none() {
        eprintln!(
            "❌ Unsupported config file extension: .{}",
            raw_extension.unwrap()
        );
        return 2;
    }

    let violations = match config_type {
        Some(ConfigType::Deno) => {
            // Load and check with unified flow (JavaScript rules injected into config)
            match mikofia_deno::load_and_check(&config_path, &check_dir).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ Failed to run checks: {}", e);
                    return 2;
                }
            }
        }
        Some(ConfigType::Json) | None => {
            // Load JSON config
            let config = match load_config(&config_path).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("❌ Failed to load config: {}", e);
                    return 2;
                }
            };

            // Create ignore matcher from config
            let ignore_matcher = match mikofia::IgnoreMatcher::new(&config.ignore) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("❌ Failed to create ignore matcher: {}", e);
                    return 2;
                }
            };

            // Run standard checks
            mikofia::check_with_ignore(&config.nodes, &check_dir, &ignore_matcher).await
        }
    };

    // Convert violations to results and report
    let results = mikofia::violations_to_results(&violations);
    let reporter = mikofia::ConsoleReporter;
    reporter.report(&results);

    // Exit with appropriate code
    if violations.is_empty() { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_type_from_extension() {
        assert_eq!(config_type_from_extension("ts"), Some(ConfigType::Deno));
        assert_eq!(config_type_from_extension("js"), Some(ConfigType::Deno));
        assert_eq!(config_type_from_extension("json"), Some(ConfigType::Json));
        assert_eq!(config_type_from_extension("txt"), None);
    }

    #[test]
    fn test_config_type_from_extension_case_insensitive() {
        assert_eq!(config_type_from_extension("TS"), Some(ConfigType::Deno));
        assert_eq!(config_type_from_extension("JS"), Some(ConfigType::Deno));
        assert_eq!(config_type_from_extension("JSON"), Some(ConfigType::Json));
    }

    #[test]
    fn test_determine_check_directory_with_some() {
        let path = PathBuf::from("/test/path");
        let result = determine_check_directory(Some(path.clone())).unwrap();
        assert_eq!(result, path);
    }

    #[test]
    fn test_determine_check_directory_with_none() {
        let result = determine_check_directory(None);
        assert!(result.is_ok());
        // Should return current directory
        assert_eq!(result.unwrap(), std::env::current_dir().unwrap());
    }
}
