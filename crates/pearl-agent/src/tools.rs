//! tools --- the agent's hands: read, edit, write, bash, under one policy.
//!
//! M7 is one agent calling tools, not many agents. The registry binds a
//! workspace (so file edits are revertible from the session) and a sandbox
//! policy (so bash is confined). The agent loop calls `call` with a tool name
//! and JSON arguments; the result is text the model reads on the next turn.

use pearl_provider::ToolSpec;
use pearl_tools::sandbox::{self, Policy};
use pearl_tools::{Baseline, EditTool, Ops, Own, ReadTool, Tool, ToolOutput, Workspace, WriteTool};
use serde_json::{Value, json};

/// The tools one agent may call, bound to a workspace and a policy.
pub struct ToolRegistry {
    workspace: Workspace,
    policy: Policy,
    /// The writes that can be taken back. Shared with the surface, so the
    /// human's undo does not wait on the loop or ask the model.
    ops: Ops,
    /// The task whose calls these are, so a file report can be per row. Empty
    /// when the caller does not say, which is every caller that is not the
    /// runner.
    task: String,
    /// Pearl's own state inside the workspace: the session log and its claim.
    /// A baseline leaves these alone, so an undo of a command cannot rewind
    /// the conversation that recorded it.
    own: Own,
}

impl ToolRegistry {
    pub fn new(root: impl AsRef<std::path::Path>, policy: Policy) -> Self {
        Self {
            workspace: Workspace::new(root),
            policy,
            ops: Ops::new(),
            task: String::new(),
            own: Own::new(),
        }
    }

    /// The workspace the file tools act on, for revert and snapshots.
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// An edit or a write, with the file's two sides recorded so `z` reaches
    /// it. A file tool that failed changed nothing, and recording it would
    /// leave an undo that does nothing.
    async fn file_tool(&mut self, name: &str, args: Value) -> ToolOutput {
        // The shell is confined by the OS; a file tool writes inside this
        // process, so the same policy has to be kept here by the tool itself.
        // Without this a read-only launch could still write, which makes the
        // policy a promise the screen prints and the code does not keep.
        if self.policy == Policy::ReadOnly {
            let notice = self.refusal_notice("the policy is read-only");
            return ToolOutput {
                content: String::from("read-only: this launch does not write files"),
                is_error: true,
                notice: Some(notice),
            };
        }
        let rel = args
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let path = self.workspace.root().join(&rel);
        let before = std::fs::read(&path).ok();
        let output = match name {
            "edit" => EditTool {
                workspace: &mut self.workspace,
            }
            .call(args)
            .await
            .unwrap_or_else(err),
            _ => WriteTool {
                workspace: &mut self.workspace,
            }
            .call(args)
            .await
            .unwrap_or_else(err),
        };
        // A refused write comes back as the filesystem's own words rather than
        // the shell's, so it is read here too: the human learns the ground was
        // the limit, not that the tool is broken.
        if output.is_error {
            let said = if output.content.contains("escapes the root") {
                Some("outside the workspace")
            } else if self.policy == Policy::ReadOnly {
                Some("the policy is read-only")
            } else {
                sandbox::refused(self.policy, &output.content)
            };
            if let Some(said) = said {
                let notice = self.refusal_notice(said);
                let mut output = output;
                output.notice = Some(notice);
                return output;
            }
        }
        if !output.is_error {
            let after = std::fs::read(&path).unwrap_or_default();
            let op = self.ops.next_op();
            self.ops.record_file(
                op,
                self.task.clone(),
                format!("{name} {rel}"),
                path,
                before,
                after,
            );
        }
        output
    }

    /// The workspace root the tools act inside, for callers that copy it.
    pub fn root(&self) -> &std::path::Path {
        self.workspace.root()
    }

    /// The paths that are pearl's own rather than the human's work. Shared,
    /// so a child's log is named once and every baseline afterwards leaves it
    /// alone -- including the parent's, which shares the directory.
    pub fn own(&self) -> &Own {
        &self.own
    }

    /// The sandbox policy its bash calls run under.
    pub fn policy(&self) -> Policy {
        self.policy
    }

    /// Say which task the next calls belong to, so the ledger can report what
    /// one row touched rather than everything the session did.
    pub fn for_task(&mut self, task: impl Into<String>) {
        self.task = task.into();
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
                "Run a shell command in the workspace (sh on Unix, cmd.exe on Windows), confined by the sandbox policy. Returns its exit code and output; revertible by op id.",
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
            "read" => ReadTool {
                workspace: &self.workspace,
            }
            .call(args)
            .await
            .unwrap_or_else(err),
            "edit" | "write" => self.file_tool(&name, args).await,
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
        let baseline = match Baseline::capture_except(self.workspace.root(), &self.own) {
            Ok(b) => b,
            Err(e) => return ToolOutput::err(format!("cannot snapshot workspace: {e}")),
        };
        let op = self.ops.next_op();

        // Redirect the command's output to a per-op file inside the root, so
        // the capture works under every sandbox and on every OS. The file is
        // named relative to the root, which is the sandbox's working
        // directory: a relative name needs no quoting on either shell. It is
        // read and removed here; it never reaches the baseline or the model.
        let capture_name = format!(".pearl-out-{op}");
        let capture = self.workspace.root().join(&capture_name);
        let mut output = self.run_capturing(command, &capture_name, &capture).await;
        let text = std::fs::read_to_string(&capture).unwrap_or_default();
        let _ = std::fs::remove_file(&capture);
        self.ops.record(op, self.task.clone(), command, baseline);

        const LIMIT: usize = 8000;
        let text = if text.len() > LIMIT {
            format!("{}\n… truncated", &text[..LIMIT])
        } else {
            text
        };
        output.content = if text.trim().is_empty() {
            format!("[op {op}] {}", output.content)
        } else {
            format!("[op {op}] {}\n{}", output.content, text.trim_end())
        };
        // The OS's refusal is in the output the model reads; the human reading
        // the report needs it named, and named with the ground it was measured
        // against, or "Operation not permitted" is a mystery with a curse in
        // it.
        // Only a failed command can have been refused: `cat` on this very
        // source prints the words and exits 0.
        if output.is_error {
            if let Some(said) = sandbox::refusal(self.policy, &output.content) {
                let notice = self.refusal_notice(said);
                output.notice = Some(notice);
            }
        }
        output
    }

    /// The line the report carries when the OS refused a write: what was said,
    /// and what the policy allows, so the next move is to write where it
    /// permits rather than to try the same place again.
    fn refusal_notice(&self, said: &str) -> String {
        let allows = match self.policy {
            Policy::ReadOnly => String::from("the policy is read-only"),
            _ => format!(
                "writes land under {} and the scratch roots",
                self.workspace.root().display()
            ),
        };
        if allows == said {
            format!("sandbox: a write was refused; {allows}")
        } else {
            format!("sandbox: a write was refused ({said}); {allows}")
        }
    }

    /// What other work changed in this workspace since `own` last did.
    pub fn elsewhere(&self, own: &[String]) -> Option<String> {
        self.ops.elsewhere(own)
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

    /// Run the command with its output captured to `capture`, a name relative
    /// to the workspace root.
    async fn run_capturing(
        &self,
        command: &str,
        _capture: &str,
        path: &std::path::Path,
    ) -> ToolOutput {
        // Windows has no shell it can rely on being installed: `sh` is a
        // third-party addendum there, and MSYS does not start inside an
        // AppContainer. The platform's own shell is the one every machine
        // has. cmd.exe parses its raw command line and does not unescape the
        // quoting CreateProcessW adds, so a command containing quotes cannot
        // survive argv; the batch file carries it instead of the command line.
        #[cfg(target_os = "windows")]
        {
            let script_name = format!("{_capture}.cmd");
            let script = self.workspace.root().join(&script_name);
            if let Err(e) = std::fs::write(&script, format!("@echo off\r\n{command}\r\n")) {
                return ToolOutput::err(format!("cannot write the op script: {e}"));
            }
            // The redirect is the child's own act, so the two files it needs
            // must be writable even when the workspace is not: a refused
            // redirect would leave the human an exit code and no words.
            let _ = std::fs::File::create(path);
            let _ =
                sandbox::permit_report(&self.workspace.root().to_string_lossy(), &[&script, path]);
            // The batch file redirects its own output; the child still gets
            // no std handle, because CreateProcessW inherits none there.
            let output = self
                .bash_into(
                    &format!("{script_name} > {_capture} 2>&1"),
                    sandbox::Sink::None,
                )
                .await;
            let _ = std::fs::remove_file(&script);
            return output;
        }
        #[cfg(not(target_os = "windows"))]
        {
            // The output is redirected by the process, not by the command
            // string: a redirection inside a command the shell cannot parse
            // never happens, so its complaint about the command would have
            // been printed on the frame instead of into the capture.
            let wrapped = format!("{{ {command} ; }}");
            self.bash_into(&wrapped, sandbox::Sink::File(path)).await
        }
    }

    /// The platform's shell, run in the workspace, its words going to `sink`
    /// and never to the terminal the human is reading.
    async fn bash_into(&self, command: &str, sink: sandbox::Sink<'_>) -> ToolOutput {
        let root = self.workspace.root().to_string_lossy().into_owned();
        #[cfg(target_os = "windows")]
        let (program, argv) = ("cmd", vec!["/C".to_string(), command.to_string()]);
        #[cfg(not(target_os = "windows"))]
        let (program, argv) = ("sh", vec!["-c".to_string(), command.to_string()]);
        let spawned = sandbox::spawn(program, &argv, self.policy, &root, sink);
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => return ToolOutput::err(format!("could not spawn: {e}")),
        };
        match child.wait_code() {
            Ok(code) => {
                let code = code.unwrap_or(0);
                ToolOutput {
                    content: format!("exit {code}"),
                    is_error: code != 0,
                    notice: None,
                }
            }
            Err(e) => ToolOutput::err(format!("wait failed: {e}")),
        }
    }
}

fn err(e: pearl_tools::ToolError) -> ToolOutput {
    ToolOutput::err(e.to_string())
}
