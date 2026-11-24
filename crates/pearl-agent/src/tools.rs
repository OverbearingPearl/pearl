//! tools --- the agent's hands: read, edit, write, bash, under one policy.
//!
//! M7 is one agent calling tools, not many agents. The registry binds a
//! workspace (so file edits are revertible from the session) and a sandbox
//! policy (so bash is confined). The agent loop calls `call` with a tool name
//! and JSON arguments; the result is text the model reads on the next turn.

use pearl_provider::ToolSpec;
use pearl_tools::sandbox::{self, Policy};
use pearl_tools::{Baseline, EditTool, Ops, ReadTool, Tool, ToolOutput, Workspace, WriteTool};
use serde_json::{json, Value};

/// The tools one agent may call, bound to a workspace and a policy.
pub struct ToolRegistry {
    workspace: Workspace,
    policy: Policy,
    /// The writes that can be taken back. Shared with the surface, so the
    /// human's undo does not wait on the loop or ask the model.
    ops: Ops,
    next_op: u64,
}

impl ToolRegistry {
    pub fn new(root: impl AsRef<std::path::Path>, policy: Policy) -> Self {
        Self { workspace: Workspace::new(root), policy, ops: Ops::new(), next_op: 0 }
    }

    /// The workspace the file tools act on, for revert and snapshots.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// The workspace root the tools act inside, for callers that copy it.
    pub fn root(&self) -> &std::path::Path {
        self.workspace.root()
    }

    /// The sandbox policy its bash calls run under.
    pub fn policy(&self) -> Policy {
        self.policy
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
                "Run a shell command in the workspace, confined by the sandbox policy. Returns its exit code and output; revertible by op id.",
                json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}),
            ),
            ToolSpec::new(
                "revert_bash",
                "Take a shell op back by its id, restoring the workspace to how it was before that op. The id is the [op N] in the op's own output.",
                json!({"type":"object","properties":{"op":{"type":"integer"}},"required":["op"]}),
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
            "bash" => self.bash_revertible(args).await,
            "revert_bash" => {
                let op = args.get("op").and_then(Value::as_u64).unwrap_or(0);
                match self.revert_bash(op) {
                    Ok(0) => ToolOutput::err(format!("op {op} has nothing to restore")),
                    Ok(n) => ToolOutput::ok(format!("op {op} reverted: {n} path(s) restored")),
                    Err(e) => ToolOutput::err(format!("op {op} could not be reverted: {e}")),
                }
            }
            other => ToolOutput::err(format!("unknown tool: {other}")),
        }
    }

    /// Run a shell command confined by the policy, capturing a baseline first
    /// so the op is revertible, and its output so the model can read it. The
    /// op id is reported so the human can revert.
    async fn bash_revertible(&mut self, args: Value) -> ToolOutput {
        let Some(command) = args.get("command").and_then(|v| v.as_str()) else {
            return ToolOutput::err("bash needs a command");
        };
        let baseline = match Baseline::capture(self.workspace.root()) {
            Ok(b) => b,
            Err(e) => return ToolOutput::err(format!("cannot snapshot workspace: {e}")),
        };
        let op = self.next_op;
        self.next_op += 1;

        // Redirect the command's output to a per-op file inside the root, so
        // the capture works under every sandbox and on every OS. The file is
        // read and removed here; it never reaches the baseline or the model.
        let capture = self.workspace.root().join(format!(".pearl-out-{op}"));
        let quoted = capture.to_string_lossy().replace('\'', "'\\''");
        let wrapped = format!("{{ {command} ; }} > '{quoted}' 2>&1");
        let mut output = self.bash(&wrapped).await;
        let text = std::fs::read_to_string(&capture).unwrap_or_default();
        let _ = std::fs::remove_file(&capture);
        self.ops.record(op, command, baseline);

        const LIMIT: usize = 8000;
        let text = if text.len() > LIMIT { format!("{}\n… truncated", &text[..LIMIT]) } else { text };
        output.content = if text.trim().is_empty() {
            format!("[op {op}] {}", output.content)
        } else {
            format!("[op {op}] {}\n{}", output.content, text.trim_end())
        };
        output
    }

    /// The ops that can still be taken back, oldest first, with what they were.
    pub fn ops(&self) -> Vec<(u64, String)> {
        self.ops.list()
    }

    /// Share the ledger with the surface, so an undo at the keyboard restores
    /// the same before-images the runner recorded.
    pub fn with_ops(mut self, ops: Ops) -> Self {
        self.ops = ops;
        self
    }

    /// The ledger this registry records into.
    pub fn ledger(&self) -> Ops {
        self.ops.clone()
    }

    /// Take the most recent op back. `None` when nothing is left to undo; the
    /// description names the command so the human knows what changed.
    pub fn revert_last(&mut self) -> std::io::Result<Option<String>> {
        self.ops.revert_last()
    }

    /// Revert a bash op by id, restoring the workspace to its pre-op state.
    /// Returns the number of paths restored; the baseline is consumed.
    pub fn revert_bash(&mut self, op: u64) -> std::io::Result<usize> {
        self.ops.revert(op)
    }

    async fn bash(&self, command: &str) -> ToolOutput {
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
