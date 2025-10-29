mod context;
mod ops;
mod runtime;

pub use context::{context_to_v8, create_context_with_fs};
pub use runtime::DenoRuntime;

use mikofia::Config;
use std::path::Path;

/// Load a TypeScript configuration file
pub async fn load_typescript_config(
    path: &Path,
) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let mut runtime = DenoRuntime::new()?;
    runtime.load_config(path).await
}
