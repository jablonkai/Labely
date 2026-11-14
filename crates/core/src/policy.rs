//! Session policy: what a frontend lets commands do (ADR-0005 safety rules).
//!
//! The CLI runs unrestricted. The MCP server restricts file access to configured roots and
//! refuses `print.*` until the user allows agent printing.

use std::path::{Component, Path, PathBuf};

use crate::command::{CommandError, ErrorCode};

/// Limits applied by [`Session::execute`](crate::Session::execute) and file commands.
#[derive(Debug, Clone)]
pub struct Policy {
    /// Directories file commands may read and write below; `None` means anywhere.
    pub file_roots: Option<Vec<PathBuf>>,
    /// Whether `print.*` commands may run.
    pub allow_print: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self::unrestricted()
    }
}

impl Policy {
    pub fn unrestricted() -> Self {
        Self {
            file_roots: None,
            allow_print: true,
        }
    }

    /// No file access outside `roots`, printing off.
    pub fn restricted(roots: Vec<PathBuf>) -> Self {
        Self {
            file_roots: Some(roots.iter().map(|r| normalize(r)).collect()),
            allow_print: false,
        }
    }

    /// Resolve `path` and check it lies under a root. `write` paths need not exist yet.
    pub fn check_path(&self, path: &str, write: bool) -> Result<PathBuf, CommandError> {
        let p = PathBuf::from(path);
        let Some(roots) = &self.file_roots else {
            return Ok(p);
        };
        let resolved = resolve(&p, write);
        if roots.iter().any(|r| resolved.starts_with(r)) {
            return Ok(p);
        }
        Err(CommandError::new(
            ErrorCode::Forbidden,
            format!("`{path}` is outside the allowed file roots"),
        )
        .with_accepted(roots.iter().map(|r| r.display().to_string()))
        .with_hint("use a path below one of the accepted roots"))
    }

    /// `forbidden` unless printing is allowed.
    pub fn check_print(&self, id: &str) -> Result<(), CommandError> {
        if self.allow_print {
            Ok(())
        } else {
            Err(CommandError::new(
                ErrorCode::Forbidden,
                format!("`{id}` is not allowed: agent printing is off"),
            )
            .with_hint(
                "ask the user to allow agent printing in the studio, or start `labely mcp \
                 --headless --allow-print`; render_preview and check still work",
            ))
        }
    }
}

/// Absolute, `..`-free path; symlinks resolved for the existing part.
fn resolve(p: &Path, write: bool) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    if write && let (Some(parent), Some(name)) = (p.parent(), p.file_name()) {
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        if let Ok(c) = parent.canonicalize() {
            return c.join(name);
        }
    }
    normalize(p)
}

/// Lexical normalisation against the current directory.
fn normalize(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots() {
        let dir = std::env::temp_dir().join(format!("labely-policy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = Policy::restricted(vec![dir.clone()]);
        let inside = dir.join("a.labely");
        assert!(p.check_path(inside.to_str().unwrap(), true).is_ok());
        let escape = dir.join("../x.labely");
        let e = p.check_path(escape.to_str().unwrap(), true).unwrap_err();
        assert_eq!(e.code, ErrorCode::Forbidden);
        assert!(p.check_path("/etc/passwd", false).is_err());
        assert!(
            Policy::unrestricted()
                .check_path("/etc/passwd", false)
                .is_ok()
        );
        assert!(p.check_print("print.submit").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
