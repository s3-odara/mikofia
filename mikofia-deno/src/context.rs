use deno_core::{serde_v8, v8};
use mikofia::EvaluationContext;

/// Convert EvaluationContext to a V8 object
/// This object will be passed to JavaScript validation rules
pub fn context_to_v8<'s>(
    scope: &mut v8::HandleScope<'s>,
    ctx: &EvaluationContext,
) -> Result<v8::Local<'s, v8::Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Use serde_v8 to convert the context to a V8 object
    let v8_value = serde_v8::to_v8(scope, ctx)?;
    Ok(v8_value)
}

/// Create a context object with filesystem operations attached
/// This adds the `fs` object with methods like readFile, readJson, exists
pub fn create_context_with_fs<'s>(
    scope: &mut v8::HandleScope<'s>,
    ctx: &EvaluationContext,
) -> Result<v8::Local<'s, v8::Object>, Box<dyn std::error::Error + Send + Sync>> {
    // First, convert the EvaluationContext to a V8 object
    let ctx_value = serde_v8::to_v8(scope, ctx)?;
    let ctx_obj: v8::Local<v8::Object> = ctx_value
        .try_into()
        .map_err(|_| io_error("Failed to convert context to object"))?;

    // Create the fs object with bound methods
    let fs_obj = create_fs_object(scope)?;

    // Attach the fs object to the context
    let fs_key = v8::String::new(scope, "fs").ok_or(io_error("Failed to create 'fs' key"))?;
    ctx_obj.set(scope, fs_key.into(), fs_obj.into());

    Ok(ctx_obj)
}

/// Create the fs object with filesystem operation methods
fn create_fs_object<'s>(
    scope: &mut v8::HandleScope<'s>,
) -> Result<v8::Local<'s, v8::Object>, Box<dyn std::error::Error + Send + Sync>> {
    let code = v8::String::new(scope, "globalThis.__mikofiaCreateFs()")
        .ok_or(io_error("Failed to create fs factory source"))?;
    let script =
        v8::Script::compile(scope, code, None).ok_or(io_error("Failed to compile fs factory"))?;
    let result = script
        .run(scope)
        .ok_or(io_error("Failed to evaluate fs factory"))?;

    result
        .try_into()
        .map_err(|_| io_error("fs factory did not return an object"))
}

fn io_error(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        message.to_string(),
    ))
}
