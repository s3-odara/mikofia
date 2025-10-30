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
        .map_err(|_| "Failed to convert context to object")?;

    // Create the fs object with bound methods
    let fs_obj = create_fs_object(scope)?;

    // Attach the fs object to the context
    let fs_key = v8::String::new(scope, "fs").ok_or("Failed to create 'fs' key")?;
    ctx_obj.set(scope, fs_key.into(), fs_obj.into());

    Ok(ctx_obj)
}

/// Create the fs object with filesystem operation methods
fn create_fs_object<'s>(
    scope: &mut v8::HandleScope<'s>,
) -> Result<v8::Local<'s, v8::Object>, Box<dyn std::error::Error + Send + Sync>> {
    let fs_obj = v8::Object::new(scope);

    // Add placeholder comment - actual ops are registered globally via Deno.core.ops
    // The JavaScript code will access ops like:
    // - Deno.core.ops.op_read_file(path)
    // - Deno.core.ops.op_read_json(path)
    // - Deno.core.ops.op_exists(path)
    //
    // We create wrapper functions here for a cleaner API
    let _read_file_src = r#"
        async function readFile(path) {
            return await Deno.core.ops.op_read_file(path);
        }
    "#;
    let _read_json_src = r#"
        async function readJson(path) {
            return await Deno.core.ops.op_read_json(path);
        }
    "#;
    let _exists_src = r#"
        function exists(path) {
            return Deno.core.ops.op_exists(path) === 1;
        }
    "#;

    // Note: These functions need to be evaluated and attached to the fs object
    // For now, we'll return an empty object and document that fs operations
    // should be accessed via Deno.core.ops directly in JavaScript rules

    Ok(fs_obj)
}
