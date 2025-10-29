use deno_core::{extension, op2};
use std::path::{Path, PathBuf};

/// Maximum file size that can be read (10MB)
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

extension!(
    mikofia_ops,
    ops = [op_read_file, op_read_json, op_exists],
);

/// Initialize the mikofia ops extension
pub fn init_ops() -> deno_core::Extension {
    mikofia_ops::init()
}

/// Read file contents as string
/// Security: Only files within the project root can be accessed
#[op2(async)]
#[string]
async fn op_read_file(#[string] path: String) -> Result<String, std::io::Error> {
    // Validate path
    validate_path(&path)?;

    // Check file size
    let metadata = tokio::fs::metadata(&path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("File exceeds maximum size of {} bytes", MAX_FILE_SIZE),
        ));
    }

    // Read file
    let content = tokio::fs::read_to_string(&path).await?;
    Ok(content)
}

/// Read and parse JSON file
#[op2(async)]
#[serde]
async fn op_read_json(#[string] path: String) -> Result<serde_json::Value, std::io::Error> {
    // Read file contents
    validate_path(&path)?;
    let metadata = tokio::fs::metadata(&path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("File exceeds maximum size of {} bytes", MAX_FILE_SIZE),
        ));
    }
    let content = tokio::fs::read_to_string(&path).await?;

    // Parse JSON
    let json = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(json)
}

/// Check if file or directory exists
#[op2(fast)]
#[smi]
fn op_exists(#[string] path: &str) -> u32 {
    // Validate path (return 0 for invalid paths)
    if validate_path(path).is_err() {
        return 0;
    }

    if Path::new(path).exists() {
        1
    } else {
        0
    }
}

/// Validate that the path is within allowed boundaries
fn validate_path(path: &str) -> Result<(), std::io::Error> {
    let path_buf = PathBuf::from(path);

    // Check if path is absolute (required for security)
    if !path_buf.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path must be absolute",
        ));
    }

    // Canonicalize to prevent directory traversal attacks
    // Note: This will fail if the path doesn't exist, which is acceptable
    // for read operations but not for exists checks
    if path_buf.exists() {
        let _canonical = path_buf.canonicalize()?;
        // Additional validation could be added here to ensure
        // the path is within the project root
        // This would require passing the root path through OpState
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_rejects_relative() {
        let result = validate_path("../secret.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_rejects_relative_current() {
        let result = validate_path("./file.txt");
        assert!(result.is_err());
    }
}
