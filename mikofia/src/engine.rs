use std::fs;
use std::path::Path;

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
    let full_path = root.join(&node.path);
    let exists = full_path.exists();

    CheckPipeline::new(node, full_path, exists)
        .check_existence()
        .check_kind()
        .check_directory_with(check_directory_violations)
        .violations()
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

    // Get defined children paths
    let defined_items: Vec<&str> = node
        .children
        .iter()
        .map(|child| child.path.as_str())
        .collect();

    // Find unlisted items
    actual_items
        .into_iter()
        .filter(|item| !defined_items.contains(&item.as_str()))
        .map(|item| Violation {
            path: format!("{}/{}", node.path, item),
            message: format!("Unlisted child item: {}", item),
        })
        .collect()
}
