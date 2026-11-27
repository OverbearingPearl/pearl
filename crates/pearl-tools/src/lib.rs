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
pub mod tool;

pub use baseline::{Baseline, Own, copy_workspace, copy_workspace_except};
pub use fs::Workspace;
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

/// Read a file inside the workspace.
pub struct ReadTool<'a> {
    pub workspace: &'a Workspace,
}

impl Tool for ReadTool<'_> {
    fn name(&self) -> &str {
        "read"
    }
    fn description(&self) -> &str {
        "Read a file inside the workspace root."
    }
    async fn call(&mut self, args: Value) -> Result<ToolOutput, ToolError> {
        let path = arg_str(&args, "path")?;
        Ok(ToolOutput::ok(self.workspace.read(&path)?))
    }
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
