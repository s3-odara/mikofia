use deno_ast::MediaType;
use deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind, error::ModuleLoaderError,
};

/// Custom module loader that supports TypeScript transpilation
pub struct TsModuleLoader;

impl TsModuleLoader {
    pub fn new() -> Self {
        Self
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

        let future = async move {
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
            let code = if should_transpile {
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

                parsed
                    .transpile(
                        &Default::default(),
                        &Default::default(),
                        &Default::default(),
                    )
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to transpile TypeScript: {}", e),
                        )
                    })?
                    .into_source()
                    .text
            } else {
                code
            };

            let module_type = match media_type {
                MediaType::Json => ModuleType::Json,
                _ => ModuleType::JavaScript,
            };

            Ok(ModuleSource::new(
                module_type,
                ModuleSourceCode::String(code.into()),
                &module_specifier,
                None,
            ))
        };

        ModuleLoadResponse::Async(Box::pin(future))
    }
}

impl Default for TsModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}
