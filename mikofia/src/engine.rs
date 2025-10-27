use std::fs;
use std::path::Path;

use crate::glob;
use crate::pipeline::{CheckPipeline, KindChecked};
use crate::types::{Node, Violation};

/// Main entry point for validation
pub fn check(nodes: &[Node], root: &Path) -> Vec<Violation> {
    nodes
        .iter()
        .flat_map(|node| check_node(node, root))
        .collect()
}

fn check_node(node: &Node, root: &Path) -> Vec<Violation> {
    // Check if path is a glob pattern
    if node.is_glob_pattern() {
        return check_glob_node(node, root);
    }

    // Regular path checking
    let full_path = root.join(&node.path);
    let exists = full_path.exists();

    CheckPipeline::new(node, full_path, exists)
        .check_existence()
        .check_kind()
        .check_directory_with(check_directory_violations)
        .violations()
}

fn check_glob_node(node: &Node, root: &Path) -> Vec<Violation> {
    // Expand glob pattern
    let matched_paths = match glob::expand_glob(&node.path, root) {
        Ok(paths) => paths,
        Err(e) => {
            return vec![Violation {
                path: node.path.clone(),
                message: e,
            }];
        }
    };

    // If required and no matches, that's a violation
    if matched_paths.is_empty() {
        if matches!(node.existence, crate::types::Existence::Required) {
            return vec![Violation {
                path: node.path.clone(),
                message: format!("No files match required pattern: {}", node.path),
            }];
        }
        return vec![];
    }

    // Check each matched path
    matched_paths
        .into_iter()
        .flat_map(|path| {
            let exists = path.exists();
            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_path_buf();

            CheckPipeline::new(node, path, exists)
                .check_existence()
                .check_kind()
                .check_directory_with(check_directory_violations)
                .violations()
                .into_iter()
                .map(move |mut v| {
                    // Update violation path to include both pattern and actual path
                    v.path = format!(
                        "{} (matched: {})",
                        node.path,
                        relative_path.display()
                    );
                    v
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Extension trait to add directory checking to KindChecked state
trait DirectoryCheck<'a> {
    fn check_directory_with<F>(self, f: F) -> CheckPipeline<'a, KindChecked>
    where
        F: FnOnce(&Node, &Path) -> Vec<Violation>;
}

impl<'a> DirectoryCheck<'a> for CheckPipeline<'a, KindChecked> {
    fn check_directory_with<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&Node, &Path) -> Vec<Violation>,
    {
        if !self.should_continue || !self.path.is_dir() {
            return self;
        }

        let new_violations = f(self.node, &self.path);
        self.violations.extend(new_violations);
        self
    }
}

fn check_directory_violations(node: &Node, dir_path: &Path) -> Vec<Violation> {
    let strict_violations = check_strict(node, dir_path);
    let children_violations = check_children(node, dir_path);

    strict_violations
        .into_iter()
        .chain(children_violations)
        .collect()
}

fn check_children(node: &Node, dir_path: &Path) -> Vec<Violation> {
    node.children
        .iter()
        .flat_map(|child| check_node(child, dir_path))
        .collect()
}

fn check_strict(node: &Node, dir_path: &Path) -> Vec<Violation> {
    if !node.is_strict() {
        return vec![];
    }

    // Get actual items in directory
    let actual_items: Vec<String> = match fs::read_dir(dir_path) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => return vec![], // Ignore read errors
    };

    // Build matchers for children (both literal paths and glob patterns)
    let matchers: Vec<_> = node
        .children
        .iter()
        .filter_map(|child| {
            if child.is_glob_pattern() {
                // Compile glob pattern
                globset::Glob::new(&child.path)
                    .ok()
                    .map(|g| (child.path.as_str(), g.compile_matcher()))
            } else {
                // For literal paths, create a simple exact matcher using glob
                globset::Glob::new(&child.path)
                    .ok()
                    .map(|g| (child.path.as_str(), g.compile_matcher()))
            }
        })
        .collect();

    // Find unlisted items
    actual_items
        .into_iter()
        .filter(|item| {
            // Check if item matches any defined pattern
            !matchers.iter().any(|(_, matcher)| matcher.is_match(item))
        })
        .map(|item| Violation {
            path: format!("{}/{}", node.path, item),
            message: format!("Unlisted child item: {}", item),
        })
        .collect()
}
