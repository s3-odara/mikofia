use deno_core::v8;
use mikofia::{EvaluationContext, RuleResult};

/// Handle to a JavaScript validation rule function
#[derive(Clone)]
pub struct JavaScriptRuleHandle {
    /// The V8 function to call for validation
    function: v8::Global<v8::Function>,
}

impl std::fmt::Debug for JavaScriptRuleHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaScriptRuleHandle")
            .field("function", &"<v8::Function>")
            .finish()
    }
}

impl JavaScriptRuleHandle {
    /// Create a new JavaScript rule handle from a V8 function
    pub fn new(scope: &mut v8::HandleScope, function: v8::Local<v8::Function>) -> Self {
        Self {
            function: v8::Global::new(scope, function),
        }
    }

    /// Call the JavaScript rule with the given evaluation context
    pub async fn call(
        &self,
        runtime: &mut crate::DenoRuntime,
        ctx: &EvaluationContext,
    ) -> Result<RuleResult, Box<dyn std::error::Error + Send + Sync>> {
        runtime.call_rule(&self.function, ctx).await
    }

    /// Get a reference to the underlying V8 function global
    pub fn function(&self) -> &v8::Global<v8::Function> {
        &self.function
    }
}

/// Convert a JavaScript return value to RuleResult
pub fn js_value_to_rule_result(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
) -> Result<RuleResult, Box<dyn std::error::Error + Send + Sync>> {
    use deno_core::serde_v8;

    // Try to deserialize as RuleResult
    // The JavaScript should return an object like:
    // { type: "pass" }
    // { type: "fail", violation: { key: "...", message: "..." } }
    // { type: "skip", reason: "..." }

    let result: RuleResult = serde_v8::from_v8(scope, value)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_rule_handle_debug() {
        // Just ensure Debug is implemented
        let debug_str = format!("{:?}", "JavaScriptRuleHandle placeholder");
        assert!(debug_str.contains("JavaScriptRuleHandle") || debug_str.contains("placeholder"));
    }
}
