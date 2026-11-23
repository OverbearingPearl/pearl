//! The finding record every check emits.
//!
//! A check prints tab-separated records on stdout and exits zero; the shell
//! orchestrator collects them, formats the human line and the GitHub
//! annotation, and decides the gate from the error count. Keeping the verdict
//! out of the check is what lets `--list` and a report-only engine reuse the
//! same machinery.

/// Severity: an error fails the gate, a warning is printed and counted apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

impl Severity {
    fn tag(self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warn => "WARN",
        }
    }
}

/// One finding: where, which check, and what is wrong.
#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub check: String,
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl Finding {
    pub fn error(check: &str, file: &str, line: usize, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            check: check.to_string(),
            file: file.to_string(),
            line,
            message: message.into(),
        }
    }

    pub fn warn(check: &str, file: &str, line: usize, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warn,
            ..Self::error(check, file, line, message)
        }
    }

    /// The tab-separated record the orchestrator parses. Tabs and newlines in
    /// a message are flattened so one finding stays one line.
    pub fn emit(&self) {
        let msg = self.message.replace(['\t', '\n', '\r'], " ");
        println!(
            "{}\t{}\t{}\t{}\t{}",
            self.severity.tag(),
            self.check,
            self.file,
            self.line,
            msg
        );
    }
}

/// Emit a batch of findings.
pub fn emit_all(findings: &[Finding]) {
    for f in findings {
        f.emit();
    }
}
