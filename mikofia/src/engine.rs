use std::collections::HashSet;
use std::path::{Path, PathBuf};

use futures::future::{LocalBoxFuture, FutureExt};

use crate::fs::{self, FileSystem, RealFileSystem};
use crate::glob;
use crate::ignore::IgnoreMatcher;
use crate::pipeline::CheckPipeline;
use crate::types::{
    EvaluationContext, Node, ParentInfo, RuleHandle, RuleResult, SiblingInfo, Violation,
};

/// Main entry point for validation
pub async fn check(nodes: &[Node], root: &Path) -> Vec<Violation> {
    check_with_fs(nodes, root, &RealFileSystem).await
}

/// Main entry point for validation with custom filesystem
pub async fn check_with_fs<F: FileSystem>(nodes: &[Node], root: &Path, fs: &F) -> Vec<Violation> {
    check_with_fs_and_ignore(nodes, root, fs, &IgnoreMatcher::empty()).await
}

/// Main entry point for validation with ignore patterns
pub async fn check_with_ignore(
    nodes: &[Node],
    root: &Path,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    check_with_fs_and_ignore(nodes, root, &RealFileSystem, ignore).await
}

/// Main entry point for validation with custom filesystem and ignore patterns
pub async fn check_with_fs_and_ignore<F: FileSystem>(
    nodes: &[Node],
    root: &Path,
    fs: &F,
    ignore: &IgnoreMatcher,
) -> Vec<Violation> {
    // Extract global ignore patterns to pass to child nodes
    let global_patterns = ignore.patterns();

    let mut all_violations = Vec::new();
    for node in nodes {
        let violations = check_node_with_patterns(node, root, root, fs, global_patterns).await;
        all_violations.extend(violations);
    }
    all_violations
}

fn check_node_with_patterns<'a, F: FileSystem>(
    node: &'a Node,
    workspace_root: &'a Path,
    root: &'a Path,
    fs: &'a F,
    parent_patterns: &'a [String],
) -> LocalBoxFuture<'a, Vec<Violation>> {
    async move {
    // Calculate this node's relative path from workspace_root
    // This is used to prefix node-level ignore patterns
    let node_relative_path = if root == workspace_root {
        // Top-level node: use node.path directly
        PathBuf::from(&node.path)
    } else {
        // Nested node: combine root's relative path with node.path
        match root.strip_prefix(workspace_root) {
            Ok(root_relative) => root_relative.join(&node.path),
            Err(_) => PathBuf::from(&node.path),
        }
    };

    // Prefix node's ignore patterns with its relative path from workspace_root
    // This ensures patterns are scoped to the node's directory
    // Example: apps/web node with ignore: ["dist"] becomes "apps/web/dist"
    let prefixed_node_patterns: Vec<String> = node
        .ignore
        .iter()
        .map(|pattern| {
            let prefixed_path = node_relative_path.join(pattern);
            prefixed_path.to_string_lossy().into_owned()
        })
        .collect();

    // Combine parent patterns with prefixed node patterns
    let pattern_sets: Vec<&[String]> = if prefixed_node_patterns.is_empty() {
        vec![parent_patterns]
    } else {
        vec![parent_patterns, &prefixed_node_patterns]
    };

    let combined_matcher = match IgnoreMatcher::from_multiple(&pattern_sets) {
        Ok(matcher) => matcher,
        Err(_e) => {
            // This should not happen because patterns are validated at config load time
            // If it does, create an empty matcher
            IgnoreMatcher::empty()
        }
    };

    // Build combined patterns for children
    // Use prefixed patterns so children inherit the correct scope
    let child_patterns: Vec<String> = parent_patterns
        .iter()
        .cloned()
        .chain(prefixed_node_patterns.iter().cloned())
        .collect();

    check_node(
        node,
        workspace_root,
        root,
        fs,
        &combined_matcher,
        &child_patterns,
    )
    .await
    }.boxed_local()
}

fn check_node<'a, F: FileSystem>(
    node: &'a Node,
    workspace_root: &'a Path,
    root: &'a Path,
    fs: &'a F,
    ignore: &'a IgnoreMatcher,
    patterns_for_children: &'a [String],
) -> LocalBoxFuture<'a, Vec<Violation>> {
    async move {
    // Check if path is a glob pattern
    if node.is_glob_pattern() {
        return check_glob_node(
            node,
            workspace_root,
            root,
            fs,
            ignore,
            patterns_for_children,
        )
        .await;
    }

    // Regular path checking
    let full_path = root.join(&node.path);
    let exists = fs.exists(&full_path);

    let mut violations = CheckPipeline::new(node, full_path.clone(), exists)
        .check_existence()
        .check_kind(fs)
        .violations();

    // Check directory violations if it's a directory
    if exists && fs.is_dir(&full_path) {
        let dir_violations = check_directory_violations(
            node,
            &full_path,
            workspace_root,
            fs,
            ignore,
            patterns_for_children,
        )
        .await;
        violations.extend(dir_violations);
    }

    // Execute custom rules if the path exists
    if exists {
        let rule_violations = execute_rules(node, &full_path, fs).await;
        violations.extend(rule_violations);
    }

    violations
    }.boxed_local()
}

fn check_glob_node<'a, F: FileSystem>(
    node: &'a Node,
    workspace_root: &'a Path,
    root: &'a Path,
    fs: &'a F,
    ignore: &'a IgnoreMatcher,
    patterns_for_children: &'a [String],
) -> LocalBoxFuture<'a, Vec<Violation>> {
    async move {
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
    let mut all_violations = Vec::new();
    for path in filtered_paths {
        let exists = fs.exists(&path);
        let display_path = to_string_path(&path);

        let mut violations = CheckPipeline::new(node, path.clone(), exists)
            .check_existence()
            .check_kind(fs)
            .violations();

        // Check directory violations if it's a directory
        if exists && fs.is_dir(&path) {
            let dir_violations = check_directory_violations(
                node,
                &path,
                workspace_root,
                fs,
                ignore,
                patterns_for_children,
            )
            .await;
            violations.extend(dir_violations);
        }

        // Execute custom rules if the path exists
        if exists {
            let rule_violations = execute_rules(node, &path, fs).await;
            violations.extend(rule_violations);
        }

        // Update violation paths
        for mut v in violations {
            v.path = display_path.clone();
            all_violations.push(v);
        }
    }

    all_violations
    }.boxed_local()
}

fn check_directory_violations<'a, F: FileSystem>(
    node: &'a Node,
    dir_path: &'a Path,
    workspace_root: &'a Path,
    fs: &'a F,
    ignore: &'a IgnoreMatcher,
    patterns_for_children: &'a [String],
) -> LocalBoxFuture<'a, Vec<Violation>> {
    async move {
    let strict_violations = check_strict(node, dir_path, workspace_root, fs, ignore);
    let children_violations =
        check_children(node, dir_path, workspace_root, fs, patterns_for_children).await;

    strict_violations
        .into_iter()
        .chain(children_violations)
        .collect()
    }.boxed_local()
}

fn check_children<'a, F: FileSystem>(
    node: &'a Node,
    dir_path: &'a Path,
    workspace_root: &'a Path,
    fs: &'a F,
    patterns_for_children: &'a [String],
) -> LocalBoxFuture<'a, Vec<Violation>> {
    async move {
    let mut all_violations = Vec::new();
    for child in &node.children {
        let violations = check_node_with_patterns(
            child,
            workspace_root,
            dir_path,
            fs,
            patterns_for_children,
        )
        .await;
        all_violations.extend(violations);
    }
    all_violations
    }.boxed_local()
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

    // Derive leading path components for children to decide which entries are allowed.
    let mut literal_children: HashSet<String> = HashSet::new();
    let mut glob_matchers = Vec::new();

    for child in &node.children {
        let first_component = std::path::Path::new(&child.path)
            .components()
            .find_map(|component| match component {
                std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                _ => None,
            })
            .unwrap_or_else(|| child.path.clone());

        if crate::glob::is_glob_pattern(&first_component) {
            match glob::build_literal_glob(&first_component) {
                Ok(glob) => glob_matchers.push(glob.compile_matcher()),
                Err(_) => {
                    literal_children.insert(first_component);
                }
            }
        } else {
            literal_children.insert(first_component);
        }
    }

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

            if literal_children.contains(item.as_str()) {
                return false;
            }

            if glob_matchers
                .iter()
                .any(|matcher| matcher.is_match(item.as_str()))
            {
                return false;
            }

            true
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
async fn execute_rules<F: FileSystem>(node: &Node, path: &Path, fs: &F) -> Vec<Violation> {
    if node.rules.is_empty() {
        return vec![];
    }

    let ctx = build_evaluation_context(path, fs);

    let mut violations = Vec::new();
    for rule in &node.rules {
        let result = match rule {
            RuleHandle::Native(native_rule) => native_rule.check(&ctx),
            RuleHandle::Async(async_rule) => async_rule.check(&ctx).await,
        };

        if let RuleResult::Fail { violation } = result {
            violations.push(violation);
        }
    }

    violations
}
