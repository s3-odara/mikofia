use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use crate::fs::FileSystem;
use crate::types::{Existence, Node, NodeKind, Violation};

/// Type state markers for compile-time step ordering
pub(crate) struct Initial;
pub(crate) struct ExistenceChecked;
pub(crate) struct KindChecked;

/// Pipeline for executing validation steps with type-safe ordering
pub(crate) struct CheckPipeline<'a, State> {
    pub(crate) node: &'a Node,
    pub(crate) path: PathBuf,
    pub(crate) exists: bool,
    pub(crate) violations: Vec<Violation>,
    pub(crate) should_continue: bool,
    pub(crate) _state: PhantomData<State>,
}

impl<'a> CheckPipeline<'a, Initial> {
    /// Create a new validation pipeline
    pub(crate) fn new(node: &'a Node, path: PathBuf, exists: bool) -> Self {
        Self {
            node,
            path,
            exists,
            violations: Vec::new(),
            should_continue: true,
            _state: PhantomData,
        }
    }

    /// Check existence requirements
    pub(crate) fn check_existence(mut self) -> CheckPipeline<'a, ExistenceChecked> {
        if !self.should_continue {
            return CheckPipeline {
                node: self.node,
                path: self.path,
                exists: self.exists,
                violations: self.violations,
                should_continue: false,
                _state: PhantomData,
            };
        }

        let violation = match self.node.existence {
            Existence::Required if !self.exists => Some(Violation {
                path: self.path.to_string_lossy().into_owned(),
                message: format!("Required item not found: {}", self.node.path),
            }),
            Existence::Absent if self.exists => Some(Violation {
                path: self.path.to_string_lossy().into_owned(),
                message: format!("Item must not exist: {}", self.node.path),
            }),
            _ => None,
        };

        if let Some(v) = violation {
            self.violations.push(v);
            self.should_continue = false;
        } else if !self.exists {
            self.should_continue = false;
        }

        CheckPipeline {
            node: self.node,
            path: self.path,
            exists: self.exists,
            violations: self.violations,
            should_continue: self.should_continue,
            _state: PhantomData,
        }
    }
}

impl<'a> CheckPipeline<'a, ExistenceChecked> {
    /// Check kind (file/directory)
    pub(crate) fn check_kind<F: FileSystem>(mut self, fs: &F) -> CheckPipeline<'a, KindChecked> {
        if !self.should_continue {
            return CheckPipeline {
                node: self.node,
                path: self.path,
                exists: self.exists,
                violations: self.violations,
                should_continue: false,
                _state: PhantomData,
            };
        }

        let is_file = !fs.is_dir(&self.path);
        let is_dir = fs.is_dir(&self.path);

        let violation = match self.node.kind {
            NodeKind::File if !is_file => Some(Violation {
                path: self.path.to_string_lossy().into_owned(),
                message: format!("Expected file, but found directory: {}", self.node.path),
            }),
            NodeKind::Directory if !is_dir => Some(Violation {
                path: self.path.to_string_lossy().into_owned(),
                message: format!("Expected directory, but found file: {}", self.node.path),
            }),
            _ => None,
        };

        if let Some(v) = violation {
            self.violations.push(v);
            self.should_continue = false;
        }

        CheckPipeline {
            node: self.node,
            path: self.path,
            exists: self.exists,
            violations: self.violations,
            should_continue: self.should_continue,
            _state: PhantomData,
        }
    }
}

impl<'a> CheckPipeline<'a, KindChecked> {
    /// Extract final violations from the pipeline
    pub(crate) fn violations(self) -> Vec<Violation> {
        self.violations
    }

    /// Run additional directory-specific checks when prior stages passed
    pub(crate) fn check_directory_with<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&Node, &Path) -> Vec<Violation>,
    {
        if !self.should_continue {
            return self;
        }

        let new_violations = f(self.node, &self.path);
        self.violations.extend(new_violations);
        self
    }
}
