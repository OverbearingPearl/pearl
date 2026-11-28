//! Given-When-Then stories for the sandbox, executed for real on each OS.
//!
//! These are not cross-compile checks: each story spawns a confined process and
//! asserts what it could and could not touch. CI runs them on macOS, Linux and
//! Windows, so the confinement is proved where it actually runs.
//!
//! Every command names its target relative to the workspace, because the
//! sandbox sets the working directory to the workspace and a relative name
//! needs no quoting. A quoted absolute path would have to survive two parsers
//! (the command line and the shell), and a test that fights its own quoting
//! proves nothing about confinement.

use pearl_tools::sandbox::{self, Policy, Sink, spawn};
use rstest::rstest;
use tempfile::TempDir;

/// A shell command that writes `text` into `target`, as (program, args) per OS.
fn write_cmd(target: &str, text: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        (
            String::from("cmd"),
            vec![String::from("/C"), format!("echo {text}> {target}")],
        )
    } else {
        (
            String::from("sh"),
            vec![
                String::from("-c"),
                format!("printf %s '{text}' > '{target}'"),
            ],
        )
    }
}

/// Run `command` in the platform's own shell, from `root`, and wait.
fn run_shell(command: &str, policy: Policy, root: &str) -> Option<i32> {
    let (program, flag) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    let mut child = spawn(
        program,
        &[flag.into(), command.into()],
        policy,
        root,
        Sink::None,
    )
    .expect("spawn");
    child.wait_code().expect("wait")
}

/// Run `command` with its words kept, and wait: the refusal has to be read.
fn run_shell_saying(
    command: &str,
    policy: Policy,
    root: &str,
    said: &std::path::Path,
) -> Option<i32> {
    let (program, flag) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    let mut child = spawn(
        program,
        &[flag.into(), command.into()],
        policy,
        root,
        Sink::File(said),
    )
    .expect("spawn");
    child.wait_code().expect("wait")
}

/// A scratch file's name relative to the workspace. The workspace sits
/// directly under the temp dir, so `..` names the scratch root without a path
/// that would have to survive two parsers.
fn scratch(relative: &str) -> String {
    if cfg!(windows) {
        format!("..\\{relative}")
    } else {
        format!("../{relative}")
    }
}

/// Spawn the command under `policy` with `root` as the workspace, and wait.
fn run(target: &str, text: &str, policy: Policy, root: &str) {
    let (program, args) = write_cmd(target, text);
    let mut child =
        sandbox::spawn(&program, &args, policy, root, sandbox::Sink::None).expect("spawn");
    child.wait_code().expect("wait");
}

#[test]
fn given_workspace_write_when_writing_inside_root_then_the_file_lands() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    run("inside.txt", "hello", Policy::WorkspaceWrite, root_str);
    let written = std::fs::read_to_string(root.path().join("inside.txt"))
        .expect("a write inside the workspace must succeed");
    assert_eq!(written.trim_end(), "hello");
}

#[test]
fn given_workspace_write_when_writing_outside_root_then_the_write_is_denied() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    // The escape target must be outside the scratch grants too: the temp tree
    // is deliberately writable, so the workspace's own parent proves nothing.
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .expect("a home");
    let escape = format!(".pearl-escape-{}.txt", std::process::id());
    let outside = std::path::Path::new(&home).join(&escape);
    let target = outside.to_string_lossy().replace('\\', "/");
    run(&target, "nope", Policy::WorkspaceWrite, root_str);
    let landed = outside.exists();
    if landed {
        let _ = std::fs::remove_file(&outside);
    }
    assert!(
        !landed,
        "a write outside the workspace must be confined away"
    );
}

#[test]
fn given_read_only_when_writing_inside_root_then_the_write_is_denied() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    run("readonly.txt", "nope", Policy::ReadOnly, root_str);
    assert!(
        !root.path().join("readonly.txt").exists(),
        "read-only must deny even a write inside the workspace"
    );
}

#[test]
fn given_full_policy_when_writing_inside_root_then_the_file_lands() {
    let root = TempDir::new().unwrap();
    let root_str = root.path().to_str().unwrap();
    run("full.txt", "free", Policy::Full, root_str);
    assert!(
        root.path().join("full.txt").exists(),
        "full policy runs as the user"
    );
}

/// Run a command and keep what it said, so a refusal is a sentence rather
/// than a number: the exit code alone cannot say which half of a two-part
/// command failed.
fn capture_shell(
    command: &str,
    policy: Policy,
    root: &str,
    capture: &std::path::Path,
) -> Option<i32> {
    let (program, args) = if cfg!(windows) {
        ("cmd", vec!["/C".to_string(), command.to_string()])
    } else {
        ("sh", vec!["-c".to_string(), command.to_string()])
    };
    let mut child = pearl_tools::sandbox::spawn(
        program,
        &args,
        policy,
        root,
        pearl_tools::sandbox::Sink::File(capture),
    )
    .unwrap();
    child.wait_code().unwrap()
}

#[rstest]
fn given_a_temp_script_when_the_agent_runs_it_then_it_runs() {
    // An agent that cannot run a scratch script cannot use the shell for
    // anything but one-liners: a compiler, a formatter, a build all arrive
    // as a file that has to be executed. The script is the platform's own
    // kind: a batch file on Windows, a shell script on Unix.
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    let script = std::env::temp_dir().join(if cfg!(windows) {
        "pearl-scratch-probe.cmd"
    } else {
        "pearl-scratch-probe.sh"
    });
    // The script is written by the account that will run it, one line at a
    // time: a file the human's account made beforehand carries no ACE for a
    // confined profile, and `copy` refuses a short-name destination outright.
    // Appending with a redirect is what the log story already proves works.
    let path = script.display();
    let command = if cfg!(windows) {
        // Two lines, and no `exit 0`: a digit standing just before a
        // redirect is a handle number to cmd, so `0>>` redirected stdin
        // instead of writing the line. A batch file that reaches its end
        // returns nought anyway.
        format!("echo @echo off> {path} && echo echo scratch-ok>> {path} && call {path}")
    } else {
        format!("printf 'exit 0\\n' > {path} && sh {path}")
    };
    let capture = dir.path().join("scratch-report.txt");
    let code = capture_shell(&command, Policy::WorkspaceWrite, root, &capture);
    let said = std::fs::read_to_string(&capture).unwrap_or_default();
    let _ = std::fs::remove_file(&script);
    assert_eq!(
        code,
        Some(0),
        "the agent could not run a temp script: {command} said {said:?}"
    );
}

#[rstest]
fn given_a_temp_log_when_the_agent_appends_to_it_then_it_is_written() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    let log = std::env::temp_dir().join("pearl-scratch-probe.log");
    let _ = std::fs::remove_file(&log);
    let probe = scratch("pearl-scratch-probe.log");
    let command = format!("echo one >> {probe} && echo two >> {probe}");
    let capture = dir.path().join("scratch-report.txt");
    let code = run_shell_saying(&command, Policy::WorkspaceWrite, root, &capture);
    let said = std::fs::read_to_string(&capture).unwrap_or_default();
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    assert_eq!(
        code,
        Some(0),
        "a temp log could not be appended to: {command} said {said:?}"
    );
    assert_eq!(
        text.lines().count(),
        2,
        "a temp log could not be appended to: {command} said {said:?}"
    );
}

#[rstest]
fn given_a_read_only_policy_when_a_temp_write_is_tried_then_it_is_still_refused() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    let target = std::env::temp_dir().join("pearl-scratch-readonly.txt");
    let _ = std::fs::remove_file(&target);
    let command = format!("echo x > {}", scratch("pearl-scratch-readonly.txt"));
    let _ = run_shell(&command, Policy::ReadOnly, root);
    assert!(!target.exists(), "read-only allowed a scratch write");
}

#[rstest]
fn given_the_scratch_roots_when_they_are_listed_then_the_temp_dir_is_among_them() {
    let roots = pearl_tools::sandbox::scratch_dirs();
    let tmp = std::env::temp_dir();
    let canonical = std::fs::canonicalize(&tmp).unwrap_or(tmp);
    assert!(
        roots.iter().any(|r| std::fs::canonicalize(r)
            .map(|c| c == canonical)
            .unwrap_or(false)),
        "the system temp dir was not a scratch root: {roots:?}"
    );
}

#[rstest]
#[case::workspace_write(Policy::WorkspaceWrite)]
#[case::read_only(Policy::ReadOnly)]
#[cfg_attr(
    windows,
    ignore = "the machine refuses NUL to an AppContainer and its DACL is not ours to change (M113)"
)]
fn given_a_command_that_throws_its_errors_away_when_it_runs_then_it_still_runs(
    #[case] policy: Policy,
) {
    // `2>/dev/null` is the commonest redirect there is, and it writes to
    // nothing: a shell that cannot open /dev/null fails a command that could
    // not have damaged anything. Observed live: `ls -la 2>/dev/null || dir`
    // exited 127 with "sh: /dev/null: Operation not permitted".
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    let command = if cfg!(windows) {
        String::from("echo kept 2>NUL && echo also")
    } else {
        String::from("echo kept 2>/dev/null && echo also >/dev/null && echo after")
    };
    let said = dir.path().join("said.txt");
    let code = run_shell_saying(&command, policy, root, &said);
    let words = std::fs::read_to_string(&said).unwrap_or_default();
    assert_eq!(
        code,
        Some(0),
        "a command writing to the null device was refused: {command:?} said {words:?}"
    );
}

#[test]
fn given_a_capture_when_the_shell_cannot_read_the_command_then_it_says_so_in_the_capture() {
    let root = TempDir::new().unwrap();
    let capture = root.path().join("said.txt");
    let args = vec!["-c".to_string(), "echo )".to_string()];
    let mut child = sandbox::spawn(
        "sh",
        &args,
        Policy::Full,
        root.path().to_str().unwrap(),
        Sink::File(&capture),
    )
    .expect("spawn");
    child.wait_code().expect("wait");
    let said = std::fs::read_to_string(&capture).unwrap_or_default();
    // The words are the shell's: dash says "Syntax error".
    assert!(
        said.to_lowercase().contains("syntax error"),
        "the capture held {said:?}"
    );
}

#[test]
fn given_a_capture_when_a_command_speaks_then_the_words_land_in_it() {
    let root = TempDir::new().unwrap();
    let capture = root.path().join("said.txt");
    let args = vec!["-c".to_string(), "echo hello; echo oops >&2".to_string()];
    let mut child = sandbox::spawn(
        "sh",
        &args,
        Policy::Full,
        root.path().to_str().unwrap(),
        Sink::File(&capture),
    )
    .expect("spawn");
    child.wait_code().expect("wait");
    let said = std::fs::read_to_string(&capture).unwrap_or_default();
    assert!(said.contains("hello"), "stdout was not captured: {said:?}");
    assert!(said.contains("oops"), "stderr was not captured: {said:?}");
}

#[rstest]
fn given_a_command_that_says_something_when_it_runs_then_the_capture_holds_it() {
    // A report nobody can read is the same as no report: on Windows no
    // handle is inherited, so the shell has to make the file itself.
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    let said = dir.path().join("said.txt");
    let code = run_shell_saying("echo said-and-done", Policy::ReadOnly, root, &said);
    let words = std::fs::read_to_string(&said).unwrap_or_default();
    assert_eq!(code, Some(0), "the command did not run: {words:?}");
    assert!(
        words.contains("said-and-done"),
        "the capture did not keep the words: {words:?}"
    );
}

#[test]
fn given_read_only_when_a_write_is_refused_then_the_words_are_read_as_the_sandbox() {
    let root = TempDir::new().unwrap();
    let capture = root.path().join("said.txt");
    let (program, args) = write_cmd("inside.txt", "x");
    let mut child = sandbox::spawn(
        &program,
        &args,
        Policy::ReadOnly,
        root.path().to_str().unwrap(),
        Sink::File(&capture),
    )
    .expect("spawn");
    let code = child.wait_code().expect("wait");
    let said = std::fs::read_to_string(&capture).unwrap_or_default();
    assert_ne!(
        code,
        Some(0),
        "a read-only policy let a write land: {said:?}"
    );
    assert!(
        sandbox::refusal(Policy::ReadOnly, &said).is_some(),
        "the refusal was not recognised, so nobody can be told about it: {said:?}"
    );
}

#[test]
fn given_a_reading_of_the_words_when_no_path_precedes_them_then_no_refusal_is_named() {
    // The shape a real denial takes is `sh: /etc/x: Permission denied` -- a
    // program, a path, the phrase closing the line. A sentence about
    // refusals, or a quoted example in a file someone read, is not that, and
    // calling it one told the human the sandbox had stopped a write that was
    // never attempted.
    for prose in [
        "the message says: Operation not permitted\n",
        "- refusals arrive as `Permission denied` on some systems\n",
        "note: Read-only file system\n",
    ] {
        assert!(
            sandbox::refusal(Policy::WorkspaceWrite, prose).is_none(),
            "prose was read as a refusal: {prose:?}"
        );
    }
}

#[test]
fn given_a_line_in_the_shells_shape_when_it_is_read_then_the_refusal_is_named() {
    let said = sandbox::refusal(
        Policy::WorkspaceWrite,
        "sh: /etc/hosts: Operation not permitted\n",
    );
    assert_eq!(said, Some("Operation not permitted"));
}

#[test]
fn given_workspace_write_when_the_write_lands_then_nothing_is_read_as_a_refusal() {
    let root = TempDir::new().unwrap();
    let capture = root.path().join("said.txt");
    let (program, args) = write_cmd("inside.txt", "x");
    let mut child = sandbox::spawn(
        &program,
        &args,
        Policy::WorkspaceWrite,
        root.path().to_str().unwrap(),
        Sink::File(&capture),
    )
    .expect("spawn");
    let code = child.wait_code().expect("wait");
    let said = std::fs::read_to_string(&capture).unwrap_or_default();
    assert_eq!(code, Some(0), "a write inside the root failed: {said:?}");
    assert!(
        sandbox::refusal(Policy::WorkspaceWrite, &said).is_none(),
        "a write that landed was called a refusal: {said:?}"
    );
}

#[rstest]
fn given_a_confined_command_when_it_is_run_twice_then_both_start_and_exit() {
    // How long a confined start takes is a property of the machine, not of
    // this program: the same two starts measured 349s and 10ms on a cold
    // Windows runner, and 416s and 168s on a loaded one, where every process
    // creation passes an inspector first. A story that asserted seconds would
    // be asserting a runner's mood, so this one prints the numbers and
    // asserts only what the product promises -- the command runs, twice, and
    // the second does not need the first's profile built again.
    //
    // The numbers are worth having in the log: they are how the twenty-minute
    // Windows job was explained, and the roadmap keeps the debt (M117).
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    let (program, flag, command) = if cfg!(windows) {
        ("cmd", "/C", "rem")
    } else {
        ("sh", "-c", "true")
    };
    let mut took = Vec::new();
    for _ in 0..2 {
        let start = std::time::Instant::now();
        let mut child = spawn(
            program,
            &[flag.into(), command.into()],
            Policy::WorkspaceWrite,
            root,
            Sink::None,
        )
        .expect("spawn");
        let code = child.wait_code().expect("wait");
        assert_eq!(code, Some(0), "a confined command did not run");
        took.push(start.elapsed());
    }
    eprintln!("confined start: first {:?}, second {:?}", took[0], took[1]);
}

/// The agent's shell runs the programs a Unix-shaped command line names:
/// `cat`, `grep`, `sed`. Each is built against a shared library that sits
/// beside it, so the loader reads that directory before the program says a
/// word. A sandbox that grants a drive root a protected ACE -- protected, so
/// it does not walk a drive's worth of objects -- reaches nothing under it,
/// and the program dies before its first byte with an exit code and no words.
#[rstest]
fn given_a_confined_command_when_it_runs_cat_then_it_reads_the_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    std::fs::write(dir.path().join("work.txt"), "the words of the file\n").expect("write");
    let said = dir.path().join("cat-report.txt");
    let code = run_shell_saying("cat work.txt", Policy::WorkspaceWrite, root, &said);
    let words = std::fs::read_to_string(&said).unwrap_or_default();
    assert_eq!(
        code,
        Some(0),
        "a confined command could not run cat: {words:?}"
    );
    assert!(words.contains("the words of the file"), "{words:?}");
}

/// The agent's shell runs programs that live outside the workspace: `git`,
/// `cargo`, a compiler. An AppContainer touches nothing it was not granted,
/// so a command naming one of those is a question about the read grants and
/// not about the command.
#[rstest]
fn given_a_command_that_runs_a_program_outside_the_root_when_it_runs_then_it_starts() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();
    let command = "git --version";
    let capture = dir.path().join("outside-report.txt");
    let code = run_shell_saying(command, Policy::WorkspaceWrite, root, &capture);
    let said = std::fs::read_to_string(&capture).unwrap_or_default();
    if code != Some(0) {
        // The confined words are usually empty here: a program that cannot
        // initialize writes nothing, and its exit code is the only thing that
        // reaches the capture. Running the same command as the user says
        // whether the program is on the machine at all, which is the
        // difference between this program's grant and this machine's PATH.
        let bare = dir.path().join("outside-bare.txt");
        let loose = run_shell_saying(command, Policy::Full, root, &bare);
        let loose_said = std::fs::read_to_string(&bare).unwrap_or_default();
        panic!(
            "a program outside the root did not start: {command} exited {code:?} saying {said:?}; \
             unconfined it exited {loose:?} saying {loose_said:?}"
        );
    }
    assert!(
        said.contains("git version"),
        "the program ran but its words were not read: {said:?}"
    );
}
