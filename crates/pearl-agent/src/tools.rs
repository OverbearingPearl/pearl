//! tools --- the agent's hands: read, edit, write, bash, under one policy.
//!
//! M7 is one agent calling tools, not many agents. The registry binds a
//! workspace (so file edits are revertible from the session) and a sandbox
//! policy (so bash is confined). The agent loop calls `call` with a tool name
//! and JSON arguments; the result is text the model reads on the next turn.

use pearl_provider::ToolSpec;
use pearl_tools::sandbox::{self, Policy};
use pearl_tools::{EditTool, ReadTool, Tool, ToolOutput, Workspace, WriteTool};
use serde_json::{json, Value};

/// The tools one agent may call, bound to a workspace and a policy.
pub struct ToolRegistry {
    workspace: Workspace,
    policy: Policy,
}

impl ToolRegistry {
    pub fn new(root: impl AsRef<std::path::Path>, policy: Policy) -> Self {
        Self { workspace: Workspace::new(root), policy }
    }

    /// The workspace the file tools act on, for revert and snapshots.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// The tool specs offered to the model: read, edit, write, bash.
    pub fn specs(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec::new(
                "read",
                "Read a file's contents, relative to the workspace root.",
                json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
            ),
            ToolSpec::new(
                "edit",
                "Replace a unique anchor text in a file. Revertible from the session.",
                json!({"type":"object","properties":{"path":{"type":"string"},"anchor":{"type":"string"},"replacement":{"type":"string"}},"required":["path","anchor","replacement"]}),
            ),
            ToolSpec::new(
                "write",
                "Write a whole file, creating or overwriting. Revertible from the session.",
                json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}),
            ),
            ToolSpec::new(
                "bash",
                "Run a shell command in the workspace, confined by the sandbox policy.",
                json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}),
            ),
        ]
    }

    /// Call a tool by name with JSON arguments. An unknown tool or a failed
    /// call returns an error output the model can read and recover from.
    pub async fn call(&mut self, name: &str, args: Value) -> ToolOutput {
        match name {
            "read" => ReadTool { workspace: &self.workspace }.call(args).await.unwrap_or_else(err),
            "edit" => EditTool { workspace: &mut self.workspace }.call(args).await.unwrap_or_else(err),
            "write" => WriteTool { workspace: &mut self.workspace }.call(args).await.unwrap_or_else(err),
            "bash" => self.bash(args).await,
            other => ToolOutput::err(format!("unknown tool: {other}")),
        }
    }

    async fn bash(&self, args: Value) -> ToolOutput {
        let Some(command) = args.get("command").and_then(|v| v.as_str()) else {
            return ToolOutput::err("bash needs a command");
        };
        let root = self.workspace.root().to_string_lossy().into_owned();
        let argv = vec!["-c".to_string(), command.to_string()];
        let spawned = sandbox::spawn("sh", &argv, self.policy, &root);
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => return ToolOutput::err(format!("could not spawn: {e}")),
        };
        match child.wait_code() {
            Ok(code) => {
                let code = code.unwrap_or(0);
                ToolOutput { content: format!("exit {code}"), is_error: code != 0 }
            }
            Err(e) => ToolOutput::err(format!("wait failed: {e}")),
        }
    }
}

fn err(e: pearl_tools::ToolError) -> ToolOutput {
    ToolOutput::err(e.to_string())
}
