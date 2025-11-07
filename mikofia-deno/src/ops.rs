use deno_core::{OpState, extension, op2};
use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Maximum file size that can be read (10MB)
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

pub(crate) struct AllowedPaths {
    roots: Vec<PathBuf>,
    token: String,
}

impl AllowedPaths {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let random: u128 = rand::Rng::r#gen(&mut rng);
        let token = format!("{:032x}", random);
        Self {
            roots: Vec::new(),
            token,
        }
    }

    fn replace_roots(&mut self, roots: &[PathBuf]) -> io::Result<()> {
        let mut canonical_roots = Vec::with_capacity(roots.len());
        for root in roots {
            let canonical = root.canonicalize().map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("Invalid project root {}: {}", root.display(), e),
                )
            })?;
            canonical_roots.push(canonical);
        }
        self.roots = canonical_roots;
        Ok(())
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    fn ensure_allowed(&self, path: &Path, allow_nonexistent: bool) -> io::Result<()> {
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Path must be absolute",
            ));
        }

        if self.roots.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "No project root configured for file access",
            ));
        }

        match path.canonicalize() {
            Ok(canonical) => {
                if self.roots.iter().any(|root| canonical.starts_with(root)) {
                    Ok(())
                } else {
                    Err(permission_error(path))
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound && allow_nonexistent => {
                if let Some(existing_ancestor) = self.find_existing_ancestor(path)?
                    && self
                        .roots
                        .iter()
                        .any(|root| existing_ancestor.starts_with(root))
                {
                    return Ok(());
                }
                Err(permission_error(path))
            }
            Err(err) => Err(err),
        }
    }

    fn find_existing_ancestor(&self, path: &Path) -> io::Result<Option<PathBuf>> {
        let mut current = path;
        while let Some(parent) = current.parent() {
            if parent.as_os_str().is_empty() {
                break;
            }
            match parent.canonicalize() {
                Ok(canonical) => return Ok(Some(canonical)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    current = parent;
                    continue;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(None)
    }
}

impl Default for AllowedPaths {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn set_allowed_roots(op_state: &mut OpState, roots: &[PathBuf]) -> io::Result<()> {
    let allowed = op_state.borrow_mut::<AllowedPaths>();
    allowed.replace_roots(roots)
}

extension!(
    mikofia_ops,
    ops = [op_read_file, op_read_json, op_exists],
    state = |state| {
        state.put(AllowedPaths::default());
    }
);

/// Initialize the mikofia ops extension
pub fn init_ops() -> deno_core::Extension {
    mikofia_ops::init()
}

/// Read file contents as string
/// Security: Only files within the project root can be accessed
#[op2(async)]
#[string]
async fn op_read_file(
    state: Rc<RefCell<OpState>>,
    #[string] token: String,
    #[string] path: String,
) -> Result<String, std::io::Error> {
    let path_buf = PathBuf::from(&path);
    validate_path(&state, &token, &path_buf, true)?;

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
async fn op_read_json(
    state: Rc<RefCell<OpState>>,
    #[string] token: String,
    #[string] path: String,
) -> Result<serde_json::Value, std::io::Error> {
    let path_buf = PathBuf::from(&path);
    // Read file contents
    validate_path(&state, &token, &path_buf, true)?;
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
fn op_exists(state: Rc<RefCell<OpState>>, #[string] token: String, #[string] path: &str) -> u32 {
    let path_buf = PathBuf::from(path);
    // Validate path (return 0 for invalid paths)
    if validate_path(&state, &token, &path_buf, true).is_err() {
        return 0;
    }

    if Path::new(path).exists() { 1 } else { 0 }
}

/// Validate that the path is within allowed boundaries
fn validate_path(
    state: &Rc<RefCell<OpState>>,
    token: &str,
    path: &Path,
    allow_nonexistent: bool,
) -> Result<(), std::io::Error> {
    let op_state = state.borrow();
    let allowed = op_state.borrow::<AllowedPaths>();
    if token != allowed.token() {
        return Err(direct_access_error());
    }
    allowed.ensure_allowed(path, allow_nonexistent)
}

fn permission_error(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("Access to {} is not allowed", path.display()),
    )
}

fn direct_access_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Direct access to Deno.core ops is disabled. Use ctx.fs helpers instead.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_rejects_relative() {
        let cwd = std::env::current_dir().unwrap();
        let mut allowed = AllowedPaths::default();
        allowed.replace_roots(&[cwd]).unwrap();
        let result = allowed.ensure_allowed(Path::new("../secret.txt"), true);
        assert!(matches!(result, Err(e) if e.kind() == std::io::ErrorKind::InvalidInput));
    }

    #[test]
    fn test_validate_path_rejects_relative_current() {
        let cwd = std::env::current_dir().unwrap();
        let mut allowed = AllowedPaths::default();
        allowed.replace_roots(&[cwd]).unwrap();
        let result = allowed.ensure_allowed(Path::new("./file.txt"), true);
        assert!(matches!(result, Err(e) if e.kind() == std::io::ErrorKind::InvalidInput));
    }
}
