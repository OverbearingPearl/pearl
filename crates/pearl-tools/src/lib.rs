//! pearl-tools --- the agent's hands: read, edit, write, and a confined shell.
//!
//! Edits are anchored by text, revertible by snapshot; the shell runs under a
//! policy that is native where the OS offers it. The queue decides what runs;
//! these are only the hands it moves.

pub mod baseline;
pub mod footprint;
pub mod fs;
pub mod images;
pub mod ops;
pub mod place;
pub mod sandbox;
pub mod search;
pub mod tool;

pub use baseline::{Baseline, Own, Seen, copy_workspace, copy_workspace_except};
pub use fs::{Window, Workspace};
pub use images::{Image, Images};
pub use ops::Ops;
pub use sandbox::Policy;
pub use tool::{Tool, ToolError, ToolOutput};

use serde_json::Value;

fn arg_str(args: &Value, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::Rejected(format!("missing argument `{key}`")))
}

/// Read a run of lines from a file inside the workspace.
pub struct ReadTool<'a> {
    pub workspace: &'a Workspace,
}

/// Lines one read shows before it stops, whatever the byte budget says.
const READ_LINES: usize = 2000;
/// Characters one line shows before it is cut. A minified bundle is one line
/// of a hundred thousand characters, and a window of lines is no budget at all
/// against it.
const LINE_CHARS: usize = 2000;

impl Tool for ReadTool<'_> {
    fn name(&self) -> &str {
        "read"
    }
    fn description(&self) -> &str {
        "Read a run of lines from a text file inside the workspace root, numbered."
    }
    async fn call(&mut self, args: Value) -> Result<ToolOutput, ToolError> {
        let path = arg_str(&args, "path")?;
        let offset = arg_num(&args, "offset").unwrap_or(1).max(1);
        let limit = arg_num(&args, "limit")
            .unwrap_or(READ_LINES)
            .clamp(1, READ_LINES);
        let window = self.workspace.read_window(&path, offset, limit)?;
        if window.total == 0 {
            return Ok(ToolOutput::ok(format!("{path} is empty")));
        }
        if window.from > window.total {
            return Err(ToolError::Rejected(format!(
                "{path} has {} lines; offset {} is past the end",
                window.total, window.from
            )));
        }
        let mut out = String::new();
        for (n, line) in window.lines.iter().enumerate() {
            let number = window.from + n;
            let shown: String = if line.chars().count() > LINE_CHARS {
                let cut: String = line.chars().take(LINE_CHARS).collect();
                format!("{cut}\u{2026}")
            } else {
                line.clone()
            };
            out.push_str(&format!("{number:>6}\t{shown}\n"));
        }
        let next = window.from + window.lines.len();
        if next <= window.total {
            // The cut says what is left and how to get it. A truncation that
            // only says "truncated" leaves the reader to guess an offset, and
            // guessing is what a shell one-liner was doing by hand.
            let why = if window.bytes {
                "byte budget"
            } else {
                "line limit"
            };
            out.push_str(&format!(
                "\u{2026} {} more lines of {} ({why}); read again with offset {next}\n",
                window.total - next + 1,
                window.total
            ));
        }
        Ok(ToolOutput::ok(out))
    }
}

/// A number a tool was asked for, absent or unusable being the same thing.
fn arg_num(args: &Value, key: &str) -> Option<usize> {
    args.get(key).and_then(Value::as_u64).map(|n| n as usize)
}

/// Edit a file by replacing a unique anchor text.
pub struct EditTool<'a> {
    pub workspace: &'a mut Workspace,
}

impl Tool for EditTool<'_> {
    fn name(&self) -> &str {
        "edit"
    }
    fn description(&self) -> &str {
        "Replace a unique anchor text in a file; revertible."
    }
    async fn call(&mut self, args: Value) -> Result<ToolOutput, ToolError> {
        let path = arg_str(&args, "path")?;
        let anchor = arg_str(&args, "anchor")?;
        let replacement = arg_str(&args, "replacement")?;
        self.workspace.edit(&path, &anchor, &replacement)?;
        Ok(ToolOutput::ok(format!("edited {path}")))
    }
}

/// Find the lines naming a thing, without running a shell.
pub struct SearchTool<'a> {
    pub workspace: &'a Workspace,
}

impl Tool for SearchTool<'_> {
    fn name(&self) -> &str {
        "search"
    }
    fn description(&self) -> &str {
        "Find lines containing text, across the workspace."
    }
    async fn call(&mut self, args: Value) -> Result<ToolOutput, ToolError> {
        let pattern = arg_str(&args, "text")?;
        let within = args.get("path").and_then(Value::as_str).map(str::to_string);
        let ignore_case = args
            .get("ignore_case")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let found = search::lines(
            self.workspace.root(),
            within.as_deref(),
            &pattern,
            ignore_case,
        )?;
        if found.total == 0 {
            return Ok(ToolOutput::ok(format!(
                "no lines contain {pattern:?} ({} files searched)",
                found.files
            )));
        }
        let mut out = found.hits.join("\n");
        out.push('\n');
        if found.total > found.hits.len() {
            out.push_str(&format!(
                "\u{2026} {} more matches; narrow the text or search one path\n",
                found.total - found.hits.len()
            ));
        }
        Ok(ToolOutput::ok(out))
    }
}

/// Write a whole file inside the workspace.
pub struct WriteTool<'a> {
    pub workspace: &'a mut Workspace,
}

impl Tool for WriteTool<'_> {
    fn name(&self) -> &str {
        "write"
    }
    fn description(&self) -> &str {
        "Write a whole file inside the workspace root; revertible."
    }
    async fn call(&mut self, args: Value) -> Result<ToolOutput, ToolError> {
        let path = arg_str(&args, "path")?;
        let content = arg_str(&args, "content")?;
        self.workspace.write(&path, &content)?;
        Ok(ToolOutput::ok(format!("wrote {path}")))
    }
}
