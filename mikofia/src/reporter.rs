use crate::types::Violation;
use std::path::PathBuf;

/// Represents a single evaluation result
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    pub path: PathBuf,
    pub violation: Option<Violation>,
}

impl EvaluationResult {
    /// Create a result with a violation
    pub fn with_violation(path: PathBuf, violation: Violation) -> Self {
        Self {
            path,
            violation: Some(violation),
        }
    }

    /// Create a result without violation (success)
    pub fn success(path: PathBuf) -> Self {
        Self {
            path,
            violation: None,
        }
    }
}

/// Trait for reporting validation results
pub trait Reporter {
    fn report(&self, results: &[EvaluationResult]);
}

/// Console reporter with emoji and formatted output
pub struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn report(&self, results: &[EvaluationResult]) {
        let violations: Vec<&EvaluationResult> = results
            .iter()
            .filter(|r| r.violation.is_some())
            .collect();

        if violations.is_empty() {
            println!("✅ All checks passed!");
            return;
        }

        println!("❌ {} violation(s) found:\n", violations.len());

        for (i, result) in violations.iter().enumerate() {
            if let Some(violation) = &result.violation {
                println!("[{}] {}", i + 1, violation.path);
                println!("    {}\n", violation.message);
            }
        }

        println!("Hint: Run 'mikofia check --help' for more information.");
    }
}

/// Convert violations to evaluation results
pub fn violations_to_results(violations: &[Violation]) -> Vec<EvaluationResult> {
    violations
        .iter()
        .map(|v| {
            let path = PathBuf::from(&v.path);
            EvaluationResult::with_violation(path, v.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_evaluation_result_with_violation() {
        let violation = Violation::new("test-key", "/path/to/file", "Test message");
        let result = EvaluationResult::with_violation(PathBuf::from("/path/to/file"), violation);
        assert!(result.violation.is_some());
    }

    #[test]
    fn test_evaluation_result_success() {
        let result = EvaluationResult::success(PathBuf::from("/path/to/file"));
        assert!(result.violation.is_none());
    }

    #[test]
    fn test_violations_to_results() {
        let violations = vec![
            Violation::new("key1", "/path/1", "Message 1"),
            Violation::new("key2", "/path/2", "Message 2"),
        ];
        let results = violations_to_results(&violations);
        assert_eq!(results.len(), 2);
        assert!(results[0].violation.is_some());
        assert!(results[1].violation.is_some());
    }

    #[test]
    fn test_console_reporter_no_violations() {
        let reporter = ConsoleReporter;
        let results = vec![];
        reporter.report(&results); // Should print success message
    }

    #[test]
    fn test_console_reporter_with_violations() {
        let reporter = ConsoleReporter;
        let violations = vec![
            Violation::new("test-key", "/project/test.txt", "Test violation"),
        ];
        let results = violations_to_results(&violations);
        reporter.report(&results); // Should print violation details
    }

    #[test]
    fn test_violation_with_args() {
        let mut args = HashMap::new();
        args.insert("filename".to_string(), serde_json::json!("test.txt"));
        let violation = Violation::with_args("unlisted-child", "/path", "Message", args);
        assert!(violation.args.is_some());
    }
}
