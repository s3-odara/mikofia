use std::path::{Path, PathBuf};

use crate::fs::{FileSystem, RealFileSystem};
use crate::glob;
use crate::pipeline::CheckPipeline;
use crate::types::{Node, Violation};

/// Main entry point for validation
pub fn check(nodes: &[Node], root: &Path) -> Vec<Violation> {
    check_with_fs(nodes, root, &RealFileSystem)
}

/// Main entry point for validation with custom filesystem
pub fn check_with_fs<F: FileSystem>(nodes: &[Node], root: &Path, fs: &F) -> Vec<Violation> {
    nodes
        .iter()
        .flat_map(|node| check_node(node, root, fs))
        .collect()
}

fn check_node<F: FileSystem>(node: &Node, root: &Path, fs: &F) -> Vec<Violation> {
    // Check if path is a glob pattern
    if node.is_glob_pattern() {
        return check_glob_node(node, root, fs);
    }

    // Regular path checking
    let full_path = root.join(&node.path);
    let exists = fs.exists(&full_path);

    CheckPipeline::new(node, full_path, exists)
        .check_existence()
        .check_kind(fs)
        .check_directory_with(|n, p| check_directory_violations(n, p, fs))
        .violations()
}

fn check_glob_node<F: FileSystem>(node: &Node, root: &Path, fs: &F) -> Vec<Violation> {
    // Expand glob pattern
    let matched_paths = match glob::expand_glob(&node.path, root, fs) {
        Ok(paths) => paths,
        Err(e) => {
            return vec![Violation {
                path: absolute_pattern(root, &node.path),
                message: e,
            }];
        }
    };

    // If required and no matches, that's a violation
    if matched_paths.is_empty() {
        if matches!(node.existence, crate::types::Existence::Required) {
            return vec![Violation {
                path: absolute_pattern(root, &node.path),
                message: format!("No files match required pattern: {}", node.path),
            }];
        }
        return vec![];
    }

    // Check each matched path
    matched_paths
        .into_iter()
        .flat_map(|path| {
            let exists = fs.exists(&path);
            let display_path = to_string_path(&path);

            CheckPipeline::new(node, path.clone(), exists)
                .check_existence()
                .check_kind(fs)
                .check_directory_with(|n, p| check_directory_violations(n, p, fs))
                .violations()
                .into_iter()
                .map(move |mut v| {
                    v.path = display_path.clone();
                    // Keep message as-is; it already references the pattern when relevant.
                    v
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn check_directory_violations<F: FileSystem>(node: &Node, dir_path: &Path, fs: &F) -> Vec<Violation> {
    let strict_violations = check_strict(node, dir_path, fs);
    let children_violations = check_children(node, dir_path, fs);

    strict_violations
        .into_iter()
        .chain(children_violations)
        .collect()
}

fn check_children<F: FileSystem>(node: &Node, dir_path: &Path, fs: &F) -> Vec<Violation> {
    node.children
        .iter()
        .flat_map(|child| check_node(child, dir_path, fs))
        .collect()
}

fn check_strict<F: FileSystem>(node: &Node, dir_path: &Path, fs: &F) -> Vec<Violation> {
    if !node.is_strict() {
        return vec![];
    }

    // Get actual items in directory
    let actual_items: Vec<String> = match fs.read_dir(dir_path) {
        Ok(items) => items,
        Err(_) => return vec![], // Ignore read errors
    };

    // Split child nodes into literal names and compiled glob matchers using iterator transforms.
    let literal_children: Vec<String> = node
        .children
        .iter()
        .filter(|child| !child.is_glob_pattern())
        .map(|child| {
            std::path::Path::new(&child.path)
                .components()
                .find_map(|component| match component {
                    std::path::Component::Normal(name) => {
                        Some(name.to_string_lossy().into_owned())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| child.path.clone())
        })
        .collect();

    let glob_matchers: Vec<_> = node
        .children
        .iter()
        .filter(|child| child.is_glob_pattern())
        .filter_map(|child| {
            globset::Glob::new(&child.path)
                .ok()
                .map(|glob| (child.path.as_str(), glob.compile_matcher()))
        })
        .collect();

    // Find unlisted items
    actual_items
        .into_iter()
        .filter(|item| {
            // Check if item matches any defined pattern
            let literal_match = literal_children.iter().any(|literal| literal == item);
            if literal_match {
                return false;
            }

            !glob_matchers
                .iter()
                .any(|(_, matcher)| matcher.is_match(item))
        })
        .map(|item| Violation {
            path: to_string_path(&dir_path.join(&item)),
            message: format!("Unlisted child item: {}", item),
        })
        .collect()
}

fn to_string_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn absolute_pattern(root: &Path, pattern: &str) -> String {
    to_string_path(&normalize_join(root, pattern))
}

fn normalize_join(root: &Path, pattern: &str) -> PathBuf {
    if root == Path::new("") {
        PathBuf::from(pattern)
    } else {
        root.join(pattern)
    }
}
