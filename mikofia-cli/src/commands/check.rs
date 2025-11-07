use std::path::{Path, PathBuf};

use mikofia::Reporter;

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

    // Load config file and run checks using unified interface
    println!("🚀 Running mikofia check...\n");
    println!("📁 Directory: {}", check_dir.display());
    println!("⚙️  Config: {}\n", config_path.display());

    // Use unified interface (automatically handles JSON and Deno configs)
    let violations = match mikofia_deno::check_from_config_path(&config_path, &check_dir).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to run checks: {}", e);
            return 2;
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
