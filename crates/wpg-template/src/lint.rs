//! Template linting (`wpg template lint`).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use wpg_core::stream::Expansion;
use wpg_core::{DiagLevel, Resolve};

use crate::compile::{analyze, apply_overrides};
use crate::Template;

/// Severity of a [`LintIssue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintLevel {
    /// The template cannot be compiled / is certainly wrong.
    Error,
    /// The template works but is probably not what was meant.
    Warning,
}

impl fmt::Display for LintLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LintLevel::Error => "error",
            LintLevel::Warning => "warning",
        })
    }
}

/// One finding of [`lint()`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintIssue {
    /// Severity.
    pub level: LintLevel,
    /// Location in the template, e.g. `layers[1].fields.ttl`.
    pub path: String,
    /// Fluent message key (`lint-*`, or a wpg-core `diag-*` key for build findings).
    pub key: String,
    /// Named Fluent arguments.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
    /// English fallback text.
    pub message: String,
}

impl LintIssue {
    pub(crate) fn new(
        level: LintLevel,
        path: impl Into<String>,
        key: &str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            level,
            path: path.into(),
            key: key.to_owned(),
            args: BTreeMap::new(),
            message: message.into(),
        }
    }

    pub(crate) fn error(path: impl Into<String>, key: &str, message: impl Into<String>) -> Self {
        Self::new(LintLevel::Error, path, key, message)
    }

    pub(crate) fn warning(path: impl Into<String>, key: &str, message: impl Into<String>) -> Self {
        Self::new(LintLevel::Warning, path, key, message)
    }

    pub(crate) fn arg(mut self, name: &str, value: impl fmt::Display) -> Self {
        self.args.insert(name.to_owned(), value.to_string());
        self
    }
}

impl fmt::Display for LintIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.level, self.path, self.message)
    }
}

/// Checks `template` against the protocol catalog `resolve`: schema version, metadata,
/// protocols / fields / values, generators, variables, layer bindings and the
/// diagnostics of the built packet (packet 0 of the stream).
///
/// Variables without a default are not required here (their values are skipped).
#[must_use]
pub fn lint(template: &Template, resolve: &dyn Resolve) -> Vec<LintIssue> {
    let a = analyze(template, None, resolve);
    let mut issues = a.issues;
    let Some(c) = a.compiled else {
        return issues;
    };
    let mut packet = c.packet.clone();
    if let Ok(exp) = Expansion::new(&c.stream, None) {
        if let Ok(ov) = exp.nth(0) {
            if let Err(e) = apply_overrides(&mut packet, &ov) {
                issues.push(LintIssue::error(
                    "stream",
                    "lint-invalid-stream",
                    e.to_string(),
                ));
            }
        }
    }
    match packet.build(resolve) {
        Err(e) => issues
            .push(LintIssue::error("layers", "lint-build-error", e.to_string()).arg("value", e)),
        Ok(built) => {
            for d in built.diagnostics {
                let level = match d.level {
                    DiagLevel::Error => LintLevel::Error,
                    DiagLevel::Warning => LintLevel::Warning,
                    DiagLevel::Info => continue,
                };
                let path = match (d.layer, &d.field) {
                    (Some(l), Some(f)) => format!("layers[{l}].fields.{f}"),
                    (Some(l), None) => format!("layers[{l}]"),
                    _ => "layers".to_owned(),
                };
                issues.push(LintIssue {
                    level,
                    path,
                    key: d.key,
                    args: d.args,
                    message: d.message,
                });
            }
        }
    }
    issues
}
