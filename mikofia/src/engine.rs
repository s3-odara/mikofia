use std::path::{Path, PathBuf};

use crate::fs::{self, FileSystem, RealFileSystem};
use crate::glob;
use crate::ignore::IgnoreMatcher;
use crate::pipeline::CheckPipeline;
use crate::types::{
    EvaluationContext, Node, ParentInfo, RuleHandle, RuleResult, SiblingInfo, Violation,
};

/// Main entry point for validation
pub fn check(nodes: &[Node], root: &Path) -> Vec<Violation> {
    check_with_fs(nodes, root, &RealFileSystem)
}

/// Main entry point for validation with custom filesystem
pub fn check_with_fs<F: FileSystem>(nodes: &[Node], root: &Path, fs: &F) -> Vec<Violation> {
    check_with_fs_and_ignore(nodes, root, fs, &IgnoreMatcher::empty())
}

/// Main entry point for validation with ignore patterns
pub fn check_with_ignore(nodes: &[Node], root: &Path, ignore: &IgnoreMatcher) -> Vec<Violation> {
    check_with_fs_and_ignore(nodes, root, &RealFileSystem, ignore)
}

/// Main entry point for validation with custom filesystem and ignore patterns
pub fn check_with_fs_and_ignore<F: FileSystem>(
    nodes: &[Node],
    root: &Path,
    fs: &F,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    nodes
        .iter()
        .flat_map(|node| check_node(node, root, root, fs, ignore))
        .collect()
}

fn check_node<F: FileSystem>(
    node: &Node,
    workspace_root: &Path,
    root: &Path,
    fs: &F,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    // Check if path is a glob pattern
    if node.is_glob_pattern() {
        return check_glob_node(node, workspace_root, root, fs, ignore);
    }

    // Regular path checking
    let full_path = root.join(&node.path);
    let exists = fs.exists(&full_path);

    let mut violations = CheckPipeline::new(node, full_path.clone(), exists)
        .check_existence()
        .check_kind(fs)
        .check_directory_with(|n, p| {
            check_directory_violations(n, p, workspace_root, fs, ignore)
        })
        .violations();

    // Execute custom rules if the path exists
    if exists {
        let rule_violations = execute_rules(node, &full_path, fs);
        violations.extend(rule_violations);
    }

    violations
}

fn check_glob_node<F: FileSystem>(
    node: &Node,
    workspace_root: &Path,
    root: &Path,
    fs: &F,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    // Expand glob pattern
    let matched_paths = match glob::expand_glob(&node.path, root, fs) {
        Ok(paths) => paths,
        Err(glob::GlobError::Walk { error, .. }) if fs::is_permission_denied(&error) => {
            return vec![Violation::new(
                "permission-denied",
                absolute_pattern(root, &node.path),
                fs::permission_denied_message(&normalize_join(root, &node.path)),
            )];
        }
        Err(err) => {
            return vec![Violation::new(
                "glob-error",
                absolute_pattern(root, &node.path),
                err.to_string(),
            )];
        }
    };

    // Filter out ignored paths
    let filtered_paths: Vec<PathBuf> = matched_paths
        .into_iter()
        .filter(|path| {
            let relative_to_root = path.strip_prefix(root);
            let relative_to_workspace = path.strip_prefix(workspace_root);
            let ignored_in_root_scope = relative_to_root
                .map(|relative| ignore.is_ignored(relative))
                .unwrap_or(false);
            let ignored_in_workspace_scope = relative_to_workspace
                .map(|relative| ignore.is_ignored(relative))
                .unwrap_or(false);

            !(ignored_in_root_scope || ignored_in_workspace_scope)
        })
        .collect();

    // If required and no matches, that's a violation
    if filtered_paths.is_empty() {
        if matches!(node.existence, crate::types::Existence::Required) {
            return vec![Violation::new(
                "no-files-match-pattern",
                absolute_pattern(root, &node.path),
                format!("No files match required pattern: {}", node.path),
            )];
        }
        return vec![];
    }

    // Check each matched path
    filtered_paths
        .into_iter()
        .flat_map(|path| {
            let exists = fs.exists(&path);
            let display_path = to_string_path(&path);

            CheckPipeline::new(node, path.clone(), exists)
                .check_existence()
                .check_kind(fs)
                .check_directory_with(|n, p| {
                    check_directory_violations(n, p, workspace_root, fs, ignore)
                })
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

fn check_directory_violations<F: FileSystem>(
    node: &Node,
    dir_path: &Path,
    workspace_root: &Path,
    fs: &F,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    let strict_violations = check_strict(node, dir_path, workspace_root, fs, ignore);
    let children_violations = check_children(node, dir_path, workspace_root, fs, ignore);

    strict_violations
        .into_iter()
        .chain(children_violations)
        .collect()
}

fn check_children<F: FileSystem>(
    node: &Node,
    dir_path: &Path,
    workspace_root: &Path,
    fs: &F,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    node.children
        .iter()
        .flat_map(|child| check_node(child, workspace_root, dir_path, fs, ignore))
        .collect()
}

fn check_strict<F: FileSystem>(
    node: &Node,
    dir_path: &Path,
    workspace_root: &Path,
    fs: &F,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    if !node.is_strict() {
        return vec![];
    }

    // Get actual items in directory
    let actual_items: Vec<String> = match fs.read_dir(dir_path) {
        Ok(items) => items,
        Err(err) => {
            if fs::is_permission_denied(&err) {
                return vec![Violation::new(
                    "permission-denied",
                    to_string_path(dir_path),
                    fs::permission_denied_message(dir_path),
                )];
            }

            return vec![];
        }
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
                    std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
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
            let full_item_path = dir_path.join(item);
            let relative_item_path = full_item_path
                .strip_prefix(workspace_root)
                .unwrap_or(&full_item_path);

            // Filter out ignored items
            if ignore.is_ignored(relative_item_path) || ignore.is_ignored_str(item) {
                return false;
            }

            // Check if item matches any defined pattern
            let literal_match = literal_children.iter().any(|literal| literal == item);
            if literal_match {
                return false;
            }

            !glob_matchers
                .iter()
                .any(|(_, matcher)| matcher.is_match(item))
        })
        .map(|item| {
            Violation::new(
                "unlisted-child",
                to_string_path(&dir_path.join(&item)),
                format!("Unlisted child item: {}", item),
            )
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

/// Build evaluation context for a file or directory
fn build_evaluation_context<F: FileSystem>(path: &Path, fs: &F) -> EvaluationContext {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());

    let parent = path.parent().and_then(|parent_path| {
        parent_path.file_name().and_then(|parent_name| {
            parent_name.to_str().map(|name_str| ParentInfo {
                path: to_string_path(parent_path),
                name: name_str.to_string(),
            })
        })
    });

    let siblings = if let Some(parent_path) = path.parent() {
        match fs.read_dir(parent_path) {
            Ok(entries) => entries
                .into_iter()
                .filter(|sibling_name| sibling_name != &name)
                .map(|sibling_name| {
                    let sibling_path = parent_path.join(&sibling_name);
                    SiblingInfo {
                        name: sibling_name,
                        is_file: !fs.is_dir(&sibling_path),
                        is_directory: fs.is_dir(&sibling_path),
                    }
                })
                .collect(),
            Err(err) if fs::is_permission_denied(&err) => vec![],
            Err(_) => vec![],
        }
    } else {
        vec![]
    };

    EvaluationContext {
        path: to_string_path(path),
        name,
        extension,
        parent,
        siblings,
    }
}

/// Execute custom rules on a path
fn execute_rules<F: FileSystem>(node: &Node, path: &Path, fs: &F) -> Vec<Violation> {
    if node.rules.is_empty() {
        return vec![];
    }

    let ctx = build_evaluation_context(path, fs);

    node.rules
        .iter()
        .filter_map(|rule| match rule {
            RuleHandle::Native(native_rule) => match native_rule.check(&ctx) {
                RuleResult::Fail { violation } => Some(violation),
                RuleResult::Pass | RuleResult::Skip { .. } => None,
            },
        })
        .collect()
}
