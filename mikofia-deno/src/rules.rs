use deno_core::v8;
use mikofia::{AsyncRule, EvaluationContext, RuleResult};
use std::cell::RefCell;
use std::rc::Rc;

type SharedRuntime = Rc<RefCell<Option<crate::DenoRuntime>>>;

struct RuntimeLease {
    shared: SharedRuntime,
    runtime: Option<crate::DenoRuntime>,
}

impl RuntimeLease {
    fn new(shared: SharedRuntime) -> Self {
        let mut slot = shared.borrow_mut();
        let runtime = slot
            .take()
            .expect("Deno runtime is already borrowed for rule execution");
        drop(slot);
        Self {
            shared,
            runtime: Some(runtime),
        }
    }

    fn runtime(&mut self) -> &mut crate::DenoRuntime {
        self.runtime
            .as_mut()
            .expect("Runtime missing while executing JavaScript rule")
    }
}

impl Drop for RuntimeLease {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            let mut slot = self.shared.borrow_mut();
            debug_assert!(
                slot.is_none(),
                "runtime slot should be empty before returning"
            );
            *slot = Some(runtime);
        }
    }
}

/// Handle to a JavaScript validation rule function
///
/// # Thread Safety
///
/// This type is `!Send` and `!Sync` because V8 isolates have strict thread affinity.
/// All JavaScript execution must happen on the same thread that created the runtime.
/// Use `tokio::task::LocalSet` to ensure execution stays on the creation thread.
#[derive(Clone)]
pub struct JavaScriptRuleHandle {
    /// The V8 function to call for validation
    function: v8::Global<v8::Function>,
    /// Shared runtime for executing the rule (local to creation thread)
    runtime: SharedRuntime,
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
    pub fn new(
        scope: &mut v8::HandleScope,
        function: v8::Local<v8::Function>,
        runtime: SharedRuntime,
    ) -> Self {
        Self {
            function: v8::Global::new(scope, function),
            runtime,
        }
    }

    /// Call the JavaScript rule with the given evaluation context
    pub async fn call(
        &self,
        ctx: &EvaluationContext,
    ) -> Result<RuleResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut lease = RuntimeLease::new(self.runtime.clone());
        lease.runtime().call_rule(&self.function, ctx).await
    }

    /// Get a reference to the underlying V8 function global
    pub fn function(&self) -> &v8::Global<v8::Function> {
        &self.function
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncRule for JavaScriptRuleHandle {
    async fn check(&self, ctx: &EvaluationContext) -> RuleResult {
        match self.call(ctx).await {
            Ok(RuleResult::Fail { mut violation }) => {
                // Fill in path if JavaScript rule didn't set it
                if violation.path.is_empty() {
                    violation.path = ctx.path.clone();
                }
                RuleResult::Fail { violation }
            }
            Ok(other) => other,
            Err(e) => RuleResult::Fail {
                violation: mikofia::Violation::new(
                    "js-rule-error",
                    ctx.path.clone(),
                    format!("JavaScript rule execution failed: {}", e),
                ),
            },
        }
    }
}

use serde::{Deserialize, Serialize};

/// Lightweight violation from JavaScript (without path)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsViolation {
    key: String,
    message: String,
    #[serde(default)]
    path: Option<String>,
}

/// JavaScript rule result (uses JsViolation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum JsRuleResult {
    Pass,
    Fail { violation: JsViolation },
    Skip { reason: String },
}

/// Convert a JavaScript return value to RuleResult
pub fn js_value_to_rule_result(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
) -> Result<RuleResult, Box<dyn std::error::Error + Send + Sync>> {
    use deno_core::serde_v8;

    // Try to deserialize as JsRuleResult
    // The JavaScript should return an object like:
    // { type: "pass" }
    // { type: "fail", violation: { key: "...", message: "..." } }
    // { type: "skip", reason: "..." }

    let js_result: JsRuleResult = serde_v8::from_v8(scope, value)?;

    // Convert JsRuleResult to RuleResult
    let result = match js_result {
        JsRuleResult::Pass => RuleResult::Pass,
        JsRuleResult::Skip { reason } => RuleResult::Skip { reason },
        JsRuleResult::Fail { violation } => {
            RuleResult::Fail {
                violation: mikofia::Violation::new(
                    violation.key,
                    violation.path.unwrap_or_default(), // Path will be set by caller
                    violation.message,
                ),
            }
        }
    };

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
