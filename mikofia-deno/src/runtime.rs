use deno_core::{JsRuntime, RuntimeOptions};
use mikofia::Config;
use std::path::Path;

/// Deno runtime wrapper for executing JavaScript/TypeScript rules
pub struct DenoRuntime {
    _js_runtime: JsRuntime,
}

impl DenoRuntime {
    /// Create a new Deno runtime with mikofia extensions
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let js_runtime = JsRuntime::new(RuntimeOptions {
            extensions: vec![crate::ops::init_ops()],
            ..Default::default()
        });

        Ok(Self {
            _js_runtime: js_runtime,
        })
    }

    /// Load a TypeScript config file and return parsed Config
    pub async fn load_config(
        &mut self,
        _path: &Path,
    ) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Implement config loading
        // 1. Convert path to module specifier
        // 2. Load the module
        // 3. Evaluate the module
        // 4. Extract default export and convert to Config
        todo!("Implement TypeScript config loading")
    }
}
