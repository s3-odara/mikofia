use deno_core::{serde_v8, v8, JsRuntime, ModuleSpecifier, RuntimeOptions};
use mikofia::{Config, EvaluationContext, RuleResult};
use std::path::Path;

/// Deno runtime wrapper for executing JavaScript/TypeScript rules
pub struct DenoRuntime {
    js_runtime: JsRuntime,
}

impl DenoRuntime {
    /// Create a new Deno runtime with mikofia extensions
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let js_runtime = JsRuntime::new(RuntimeOptions {
            extensions: vec![crate::ops::init_ops()],
            ..Default::default()
        });

        Ok(Self { js_runtime })
    }

    /// Load a TypeScript config file and return parsed Config
    pub async fn load_config(
        &mut self,
        path: &Path,
    ) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
        // Convert path to module specifier
        let module_specifier = ModuleSpecifier::from_file_path(path)
            .map_err(|_| "Invalid file path")?;

        // Load the module
        let module_id = self
            .js_runtime
            .load_main_es_module(&module_specifier)
            .await?;

        // Evaluate the module
        let result = self.js_runtime.mod_evaluate(module_id);
        self.js_runtime.run_event_loop(Default::default()).await?;
        result.await?;

        // Extract the default export and convert to Config
        let config_json = self.get_default_export()?;
        let config: Config = serde_json::from_value(config_json)?;

        Ok(config)
    }

    /// Get the default export from the most recently loaded module
    fn get_default_export(&mut self) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        // Execute script to get default export
        let value_global = self.js_runtime.execute_script(
            "<get_default_export>",
            r#"
                (function() {
                    // Access the module namespace
                    // This assumes the default export is available
                    // We'll need to improve this to properly access module exports
                    return globalThis.__mikofiaConfig || {};
                })()
            "#,
        )?;

        // Convert V8 value to JSON
        let scope = &mut self.js_runtime.handle_scope();
        let local_value = deno_core::v8::Local::new(scope, value_global);
        let json_value: serde_json::Value = serde_v8::from_v8(scope, local_value)?;

        Ok(json_value)
    }

    /// Execute JavaScript code and return the result
    pub fn execute_script(
        &mut self,
        name: &'static str,
        source: &'static str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let value_global = self.js_runtime.execute_script(name, source)?;
        let scope = &mut self.js_runtime.handle_scope();
        let local_value = deno_core::v8::Local::new(scope, value_global);
        let json_value: serde_json::Value = serde_v8::from_v8(scope, local_value)?;
        Ok(json_value)
    }

    /// Call a JavaScript rule function with the given context
    pub async fn call_rule(
        &mut self,
        function: &v8::Global<v8::Function>,
        ctx: &EvaluationContext,
    ) -> Result<RuleResult, Box<dyn std::error::Error + Send + Sync>> {
        // Convert context to V8 object in a separate scope
        let ctx_global = {
            let scope = &mut self.js_runtime.handle_scope();
            let ctx_value = crate::context::context_to_v8(scope, ctx)?;
            v8::Global::new(scope, ctx_value)
        };

        // Call the function with the context as argument
        let result_future = self.js_runtime.call_with_args(function, &[ctx_global]);

        // Run event loop and await result
        let result_global = result_future.await?;

        // Convert result to RuleResult in a separate scope
        let rule_result = {
            let scope = &mut self.js_runtime.handle_scope();
            let result_local = v8::Local::new(scope, result_global);
            crate::rules::js_value_to_rule_result(scope, result_local)?
        };

        Ok(rule_result)
    }
}
