use deno_ast::MediaType;
use deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind, error::ModuleLoaderError,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
struct CachedModule {
    code: Arc<str>,
    module_type: ModuleType,
}

/// Custom module loader that supports TypeScript transpilation
pub struct TsModuleLoader {
    cache: Arc<RwLock<HashMap<ModuleSpecifier, CachedModule>>>,
}

impl TsModuleLoader {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ModuleLoader for TsModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        deno_core::resolve_import(specifier, referrer).map_err(|e| ModuleLoaderError::from(e))
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let module_specifier = module_specifier.clone();
        let cache = self.cache.clone();

        let future = async move {
            if let Some(cached) = {
                let guard = cache.read().expect("ts loader cache poisoned");
                guard.get(&module_specifier).cloned()
            } {
                return Ok(ModuleSource::new(
                    cached.module_type,
                    ModuleSourceCode::String(cached.code.clone().into()),
                    &module_specifier,
                    None,
                ));
            }

            // Convert URL to file path
            let path = module_specifier.to_file_path().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Invalid file path: {}", module_specifier),
                )
            })?;

            // Determine media type from extension
            let media_type = MediaType::from_path(&path);

            // Check if transpilation is needed
            let should_transpile = matches!(
                media_type,
                MediaType::TypeScript
                    | MediaType::Tsx
                    | MediaType::Jsx
                    | MediaType::Mts
                    | MediaType::Cts
                    | MediaType::Dts
                    | MediaType::Dmts
                    | MediaType::Dcts
            );

            // Read file contents
            let code = std::fs::read_to_string(&path).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to read file {}: {}", path.display(), e),
                )
            })?;

            // Transpile if needed
            let code: Arc<str> = if should_transpile {
                let parsed = deno_ast::parse_module(deno_ast::ParseParams {
                    specifier: module_specifier.clone(),
                    text: code.into(),
                    media_type,
                    capture_tokens: false,
                    scope_analysis: false,
                    maybe_syntax: None,
                })
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to parse TypeScript: {}", e),
                    )
                })?;

                // Configure emit options for better debugging support
                // - Inline source maps for mapping JS errors back to TS source
                // - Include original TypeScript source in the source map
                let emit_options = deno_ast::EmitOptions {
                    source_map: deno_ast::SourceMapOption::Inline,
                    inline_sources: true,
                    ..Default::default()
                };

                let transpiled = parsed
                    .transpile(&Default::default(), &Default::default(), &emit_options)
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to transpile TypeScript: {}", e),
                        )
                    })?
                    .into_source();

                Arc::<str>::from(transpiled.text)
            } else {
                Arc::<str>::from(code)
            };

            let module_type = match media_type {
                MediaType::Json => ModuleType::Json,
                _ => ModuleType::JavaScript,
            };

            let module_source = ModuleSource::new(
                module_type.clone(),
                ModuleSourceCode::String(code.clone().into()),
                &module_specifier,
                None,
            );

            {
                let mut guard = cache.write().expect("ts loader cache poisoned");
                guard.insert(module_specifier.clone(), CachedModule { code, module_type });
            }

            Ok(module_source)
        };

        ModuleLoadResponse::Async(Box::pin(future))
    }
}

impl Default for TsModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deno_core::ModuleLoader;
    use tempfile::TempDir;

    async fn load_module(loader: &TsModuleLoader, specifier: &ModuleSpecifier) -> ModuleSource {
        match loader.load(specifier, None, false, RequestedModuleType::None) {
            ModuleLoadResponse::Sync(result) => result.expect("sync load failed"),
            ModuleLoadResponse::Async(fut) => fut.await.expect("async load failed"),
        }
    }

    #[tokio::test]
    async fn returns_cached_module_when_source_removed() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("config.ts");
        std::fs::write(
            &file_path,
            r#"
                export default {
                    nodes: [],
                };
            "#,
        )
        .unwrap();

        let specifier = ModuleSpecifier::from_file_path(&file_path).unwrap();
        let loader = TsModuleLoader::new();

        let first = load_module(&loader, &specifier).await;
        let first_bytes = first.code.as_bytes().to_vec();

        // Remove the file to ensure the second load must hit the cache.
        std::fs::remove_file(&file_path).unwrap();

        let second = load_module(&loader, &specifier).await;
        let second_bytes = second.code.as_bytes().to_vec();

        assert_eq!(
            first.module_type, second.module_type,
            "cached module should preserve module type"
        );
        assert_eq!(
            first_bytes, second_bytes,
            "cached module should reuse transpiled source"
        );
    }

    #[tokio::test]
    async fn transpiled_code_includes_inline_source_map() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.ts");
        std::fs::write(
            &file_path,
            r#"
                interface Config {
                    nodes: string[];
                }
                const config: Config = {
                    nodes: ["test"],
                };
                export default config;
            "#,
        )
        .unwrap();

        let specifier = ModuleSpecifier::from_file_path(&file_path).unwrap();
        let loader = TsModuleLoader::new();

        let module = load_module(&loader, &specifier).await;
        let code = std::str::from_utf8(module.code.as_bytes()).unwrap();

        // Check that inline source map is present
        assert!(
            code.contains("//# sourceMappingURL=data:application/json;base64,"),
            "Transpiled code should contain inline source map"
        );

        // Verify it's a base64-encoded JSON source map
        if let Some(source_map_line) = code
            .lines()
            .find(|line| line.starts_with("//# sourceMappingURL="))
        {
            assert!(
                source_map_line.contains("data:application/json;base64,"),
                "Source map should be base64-encoded"
            );
        } else {
            panic!("Source map URL comment not found");
        }
    }
}
