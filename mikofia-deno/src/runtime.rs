use deno_core::{
    FsModuleLoader, JsRuntime, ModuleId, ModuleSpecifier, RuntimeOptions, serde_v8, v8,
};
use mikofia::{Config, EvaluationContext, RuleResult};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Deno runtime wrapper for executing JavaScript/TypeScript rules
pub struct DenoRuntime {
    js_runtime: JsRuntime,
}

impl DenoRuntime {
    /// Create a new Deno runtime with mikofia extensions
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut js_runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(Rc::new(FsModuleLoader)),
            extensions: vec![crate::ops::init_ops()],
            ..Default::default()
        });

        Self::initialize_js_environment(&mut js_runtime)?;

        Ok(Self { js_runtime })
    }

    /// Restrict filesystem operations to the provided root directories.
    pub fn set_allowed_roots<I, P>(&mut self, roots: I) -> io::Result<()>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let collected: Vec<PathBuf> = roots
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();
        if collected.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "At least one project root must be provided",
            ));
        }

        let op_state_rc = self.js_runtime.op_state();
        {
            let mut op_state = op_state_rc.borrow_mut();
            crate::ops::set_allowed_roots(&mut op_state, &collected)?;
        }
        Ok(())
    }

    /// Load a TypeScript config file and return parsed Config
    pub async fn load_config(
        &mut self,
        path: &Path,
    ) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = path.parent() {
            self.set_allowed_roots([parent])
                .map_err(|e| format!("Failed to configure allowed paths for config: {}", e))?;
        }

        // Convert path to module specifier
        let module_specifier =
            ModuleSpecifier::from_file_path(path).map_err(|_| "Invalid file path")?;

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
        let config = self.get_default_export(module_id)?;

        Ok(config)
    }

    /// Load config with JavaScript rule functions extracted
    pub async fn load_config_with_rules(
        &mut self,
        path: &Path,
    ) -> Result<
        (
            Config,
            Vec<(String, Vec<crate::rules::JavaScriptRuleHandle>)>,
        ),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        if let Some(parent) = path.parent() {
            self.set_allowed_roots([parent])
                .map_err(|e| format!("Failed to configure allowed paths for config: {}", e))?;
        }

        // Convert path to module specifier
        let module_specifier =
            ModuleSpecifier::from_file_path(path).map_err(|_| "Invalid file path")?;

        // Load the module
        let module_id = self
            .js_runtime
            .load_main_es_module(&module_specifier)
            .await?;

        // Evaluate the module
        let result = self.js_runtime.mod_evaluate(module_id);
        self.js_runtime.run_event_loop(Default::default()).await?;
        result.await?;

        // Extract config and rules
        let (config, rules_map) = self.extract_config_and_rules(module_id)?;

        Ok((config, rules_map))
    }

    /// Get the default export from a loaded module
    fn get_default_export(
        &mut self,
        module_id: ModuleId,
    ) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
        // Get the module namespace object
        let module_namespace = self.js_runtime.get_module_namespace(module_id)?;

        // Access the "default" property from the namespace
        let scope = &mut self.js_runtime.handle_scope();
        let module_namespace_local = v8::Local::new(scope, module_namespace);

        let default_key =
            v8::String::new(scope, "default").ok_or("Failed to create 'default' string")?;

        let default_export = module_namespace_local
            .get(scope, default_key.into())
            .ok_or("No default export found in module")?;

        // Convert V8 value to Config
        let config: Config = serde_v8::from_v8(scope, default_export)?;

        Ok(config)
    }

    /// Extract config and rule functions from module
    fn extract_config_and_rules(
        &mut self,
        module_id: ModuleId,
    ) -> Result<
        (
            Config,
            Vec<(String, Vec<crate::rules::JavaScriptRuleHandle>)>,
        ),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        // Get the module namespace object
        let module_namespace = self.js_runtime.get_module_namespace(module_id)?;

        // First, extract the config
        let config = {
            let scope = &mut self.js_runtime.handle_scope();
            let module_namespace_local = v8::Local::new(scope, module_namespace.clone());

            let default_key =
                v8::String::new(scope, "default").ok_or("Failed to create 'default' string")?;

            let default_export = module_namespace_local
                .get(scope, default_key.into())
                .ok_or("No default export found in module")?;

            // Convert V8 value to Config
            serde_v8::from_v8::<Config>(scope, default_export)?
        };

        // Then, extract rules in a separate scope
        let rules_map = {
            let scope = &mut self.js_runtime.handle_scope();
            let module_namespace_local = v8::Local::new(scope, module_namespace);

            let default_key =
                v8::String::new(scope, "default").ok_or("Failed to create 'default' string")?;

            let default_export = module_namespace_local
                .get(scope, default_key.into())
                .ok_or("No default export found in module")?;

            Self::extract_rules_from_nodes(scope, default_export)?
        };

        Ok((config, rules_map))
    }

    /// Extract rule functions from nodes in the config
    fn extract_rules_from_nodes(
        scope: &mut v8::HandleScope,
        config_obj: v8::Local<v8::Value>,
    ) -> Result<
        Vec<(String, Vec<crate::rules::JavaScriptRuleHandle>)>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let mut rules_map = Vec::new();

        // Get the config object
        let config_obj: v8::Local<v8::Object> = config_obj
            .try_into()
            .map_err(|_| "Config is not an object")?;

        // Get the nodes array
        let nodes_key = v8::String::new(scope, "nodes").ok_or("Failed to create 'nodes' string")?;
        let nodes = config_obj
            .get(scope, nodes_key.into())
            .ok_or("No nodes property")?;

        if nodes.is_array() {
            let nodes_array: v8::Local<v8::Array> = nodes.try_into().unwrap();
            let len = nodes_array.length();

            for i in 0..len {
                let node = nodes_array
                    .get_index(scope, i)
                    .ok_or("Failed to get node")?;
                Self::extract_rules_from_node(scope, node, "", &mut rules_map)?;
            }
        }

        Ok(rules_map)
    }

    /// Recursively extract rules from a node and its children
    fn extract_rules_from_node(
        scope: &mut v8::HandleScope,
        node: v8::Local<v8::Value>,
        parent_path: &str,
        rules_map: &mut Vec<(String, Vec<crate::rules::JavaScriptRuleHandle>)>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let node_obj: v8::Local<v8::Object> = match node.try_into() {
            Ok(obj) => obj,
            Err(_) => return Ok(()), // Skip non-object nodes
        };

        // Get path
        let path_key = v8::String::new(scope, "path").ok_or("Failed to create 'path' string")?;
        let path = node_obj
            .get(scope, path_key.into())
            .ok_or("No path property")?;
        let path_str: String = serde_v8::from_v8(scope, path)?;

        let full_path = if parent_path.is_empty() {
            path_str.clone()
        } else {
            format!("{}/{}", parent_path, path_str)
        };

        // Extract rules from this node
        let rules_key = v8::String::new(scope, "rules").ok_or("Failed to create 'rules' string")?;
        if let Some(rules) = node_obj.get(scope, rules_key.into()) {
            if rules.is_array() {
                let rules_array: v8::Local<v8::Array> = rules.try_into().unwrap();
                let len = rules_array.length();
                let mut node_rules = Vec::new();

                for i in 0..len {
                    if let Some(rule_fn) = rules_array.get_index(scope, i) {
                        if rule_fn.is_function() {
                            let func: v8::Local<v8::Function> = rule_fn.try_into().unwrap();
                            let handle = crate::rules::JavaScriptRuleHandle::new(scope, func);
                            node_rules.push(handle);
                        }
                    }
                }

                if !node_rules.is_empty() {
                    rules_map.push((full_path.clone(), node_rules));
                }
            }
        }

        // Process children
        let children_key =
            v8::String::new(scope, "children").ok_or("Failed to create 'children' string")?;
        if let Some(children) = node_obj.get(scope, children_key.into()) {
            if children.is_array() {
                let children_array: v8::Local<v8::Array> = children.try_into().unwrap();
                let len = children_array.length();

                for i in 0..len {
                    if let Some(child) = children_array.get_index(scope, i) {
                        Self::extract_rules_from_node(scope, child, &full_path, rules_map)?;
                    }
                }
            }
        }

        Ok(())
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
            let ctx_object = crate::context::create_context_with_fs(scope, ctx)?;
            let ctx_value: v8::Local<v8::Value> = ctx_object.into();
            v8::Global::new(scope, ctx_value)
        };

        // Call the function with the context as argument
        let result_future = self.js_runtime.call_with_args(function, &[ctx_global]);

        // Run event loop with the promise to ensure it resolves
        let result_global = self
            .js_runtime
            .with_event_loop_promise(result_future, Default::default())
            .await?;

        // Convert result to RuleResult in a separate scope
        let rule_result = {
            let scope = &mut self.js_runtime.handle_scope();
            let result_local = v8::Local::new(scope, result_global);
            crate::rules::js_value_to_rule_result(scope, result_local)?
        };

        Ok(rule_result)
    }

    fn initialize_js_environment(
        js_runtime: &mut JsRuntime,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let token = {
            let op_state_rc = js_runtime.op_state();
            let op_state = op_state_rc.borrow();
            let allowed_paths = op_state.borrow::<crate::ops::AllowedPaths>();
            allowed_paths.token().to_owned()
        };

        let token_literal = serde_json::to_string(&token)?;

        let init_source = format!(
            r#"(function (globalThis) {{
                const ops = Deno.core.ops;
                const TOKEN = {token_literal};

                function ensureString(path) {{
                    if (typeof path !== "string") {{
                        throw new TypeError("Path must be a string");
                    }}
                }}

                function createFs() {{
                    return Object.freeze({{
                        async readFile(path) {{
                            ensureString(path);
                            return await ops.op_read_file(TOKEN, path);
                        }},
                        async readJson(path) {{
                            ensureString(path);
                            return await ops.op_read_json(TOKEN, path);
                        }},
                        exists(path) {{
                            ensureString(path);
                            return ops.op_exists(TOKEN, path) === 1;
                        }}
                    }});
                }}

                Object.defineProperty(globalThis, "__mikofiaCreateFs", {{
                    value: createFs,
                    writable: false,
                    enumerable: false,
                    configurable: false,
                }});

                const ERROR_MESSAGE = "Direct access to Deno.core ops is disabled. Use ctx.fs helpers instead.";

                const originalReadFile = ops.op_read_file;
                Object.defineProperty(ops, "op_read_file", {{
                    value(token, path) {{
                        if (token !== TOKEN || typeof path !== "string") {{
                            throw new Error(ERROR_MESSAGE);
                        }}
                        return originalReadFile(token, path);
                    }},
                    writable: false,
                    enumerable: false,
                    configurable: false,
                }});

                const originalReadJson = ops.op_read_json;
                Object.defineProperty(ops, "op_read_json", {{
                    value(token, path) {{
                        if (token !== TOKEN || typeof path !== "string") {{
                            throw new Error(ERROR_MESSAGE);
                        }}
                        return originalReadJson(token, path);
                    }},
                    writable: false,
                    enumerable: false,
                    configurable: false,
                }});

                const originalExists = ops.op_exists;
                Object.defineProperty(ops, "op_exists", {{
                    value(token, path) {{
                        if (token !== TOKEN || typeof path !== "string") {{
                            throw new Error(ERROR_MESSAGE);
                        }}
                        return originalExists(token, path);
                    }},
                    writable: false,
                    enumerable: false,
                    configurable: false,
                }});
            }})(globalThis);"#,
            token_literal = token_literal
        );

        js_runtime.execute_script("<mikofia:init>", init_source)?;

        Ok(())
    }
}
