use std::path::{Path, PathBuf};

use crate::templates::{ConfigFormat, get_template};

/// Pure function to determine the output directory
fn determine_output_directory(dir: Option<PathBuf>) -> Result<PathBuf, std::io::Error> {
    match dir {
        Some(d) => Ok(d),
        None => std::env::current_dir(),
    }
}

/// Pure function to determine the output file path
fn determine_output_path(dir: &Path, config: Option<PathBuf>, format: ConfigFormat) -> PathBuf {
    match config {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                dir.join(path)
            }
        }
        None => dir.join(format.default_filename()),
    }
}

/// Check if any config file already exists in the directory
fn find_existing_config(dir: &Path) -> Option<PathBuf> {
    let candidates = [
        "mikofia.config.ts",
        "mikofia.config.js",
        "mikofia.config.json",
    ];

    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
}

/// Run the init command
pub async fn run(
    format: ConfigFormat,
    config: Option<PathBuf>,
    dir: Option<PathBuf>,
    force: bool,
) -> i32 {
    // Determine output directory
    let output_dir = match determine_output_directory(dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("❌ Failed to determine output directory: {}", e);
            return 2;
        }
    };

    // Determine output file path
    let output_path = determine_output_path(&output_dir, config.clone(), format);

    // Check for existing config files
    let existing = if config.is_none() {
        // Only check for any existing config if user didn't specify explicit path
        find_existing_config(&output_dir)
    } else {
        // If user specified explicit path, only check that specific file
        if output_path.exists() {
            Some(output_path.clone())
        } else {
            None
        }
    };

    if let Some(existing_path) = existing {
        if !force {
            eprintln!("❌ Config file already exists: {}", existing_path.display());
            eprintln!("   Use --force to overwrite");
            return 3;
        } else {
            println!(
                "⚠️  Overwriting existing config: {}",
                existing_path.display()
            );
        }
    }

    // Generate template
    let template = get_template(format);

    // Create parent directory if it doesn't exist
    if let Some(parent) = output_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("❌ Failed to create directory: {}", e);
        return 2;
    }

    // Write file
    if let Err(e) = std::fs::write(&output_path, template) {
        eprintln!("❌ Failed to write config file: {}", e);
        return 2;
    }

    println!("✅ Created config file: {}", output_path.display());

    // Print additional help for JSON format
    if matches!(format, ConfigFormat::Json) {
        println!("\n💡 You can now customize your config file.");
        println!("   For TypeScript/JavaScript configs with custom rules, use:");
        println!("   mikofia init --format ts");
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_determine_output_directory_with_some() {
        let path = PathBuf::from("/test/path");
        let result = determine_output_directory(Some(path.clone())).unwrap();
        assert_eq!(result, path);
    }

    #[test]
    fn test_determine_output_directory_with_none() {
        let result = determine_output_directory(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), std::env::current_dir().unwrap());
    }

    #[test]
    fn test_determine_output_path_default() {
        let dir = PathBuf::from("/test");
        let result = determine_output_path(&dir, None, ConfigFormat::Ts);
        assert_eq!(result, PathBuf::from("/test/mikofia.config.ts"));
    }

    #[test]
    fn test_determine_output_path_custom_relative() {
        let dir = PathBuf::from("/test");
        let config = Some(PathBuf::from("custom.ts"));
        let result = determine_output_path(&dir, config, ConfigFormat::Ts);
        assert_eq!(result, PathBuf::from("/test/custom.ts"));
    }

    #[test]
    fn test_determine_output_path_custom_absolute() {
        let dir = PathBuf::from("/test");
        let config = Some(PathBuf::from("/other/custom.ts"));
        let result = determine_output_path(&dir, config, ConfigFormat::Ts);
        assert_eq!(result, PathBuf::from("/other/custom.ts"));
    }
}
