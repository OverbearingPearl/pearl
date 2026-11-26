//! Stories for the command line: what a set of arguments asks for.
//!
//! The plan is the whole of the decision the binary makes before it enters a
//! surface or a run, so it is a value a story can read.

use std::path::PathBuf;

use pearl::Plan;
use rstest::rstest;

fn plan(args: &[&str]) -> Result<Plan, clap::Error> {
    pearl::plan(args.iter().map(|a| a.to_string()))
}

#[rstest]
fn given_no_subcommand_when_planned_then_the_surface_opens() {
    assert_eq!(
        plan(&["pearl"]).unwrap(),
        Plan::Surface {
            session: "pearl-session.jsonl".into(),
            resumed: true
        }
    );
}

// The surface writes the conversation it carries, and a run that is not the
// human's own -- a check, a driven terminal, a second window -- must be able to
// put it somewhere else. The default file sits in the working directory, which
// in this repository is the repository: a test that forgot the flag would
// append its tasks to a tracked file, and a human's next question would be
// answered against them.
#[rstest]
fn given_a_named_session_when_the_surface_is_planned_then_it_is_the_file_it_writes() {
    assert_eq!(
        plan(&["pearl", "queue", "--session", "/tmp/other.jsonl"]).unwrap(),
        Plan::Surface {
            session: "/tmp/other.jsonl".into(),
            resumed: true
        }
    );
}

#[rstest]
fn given_the_queue_subcommand_when_planned_then_it_opens_the_default_session() {
    assert_eq!(
        plan(&["pearl", "queue"]).unwrap(),
        Plan::Surface {
            session: "pearl-session.jsonl".into(),
            resumed: true
        }
    );
}

#[rstest]
fn given_the_run_subcommand_when_planned_then_the_task_is_taken_with_the_defaults() {
    let drawn = plan(&["pearl", "run", "fix the tests"]).unwrap();
    let Plan::Run {
        task,
        model,
        session,
        resumed,
    } = drawn
    else {
        panic!("a run was not planned: {drawn:?}");
    };
    assert_eq!(task, "fix the tests");
    assert_eq!(model, "deepseek-chat");
    assert_eq!(session, PathBuf::from("pearl-session.jsonl"));
    assert!(resumed, "the default is to carry on: {resumed}");
}

#[rstest]
fn given_the_run_subcommand_with_flags_when_planned_then_the_flags_win() {
    let drawn = plan(&[
        "pearl",
        "run",
        "fix",
        "--model",
        "qwen",
        "--session",
        "s.jsonl",
    ])
    .unwrap();
    assert_eq!(
        drawn,
        Plan::Run {
            task: "fix".into(),
            model: "qwen".into(),
            session: "s.jsonl".into(),
            resumed: true
        }
    );
}

#[rstest]
fn given_an_unknown_subcommand_when_planned_then_it_is_an_error() {
    assert!(plan(&["pearl", "sing"]).is_err());
}

// The surface is the program, so the file it writes has to be nameable without
// asking for a subcommand that only means "the default". Without this the only
// way to start a fresh conversation was to know that `queue` takes the flag:
// the natural `pearl --session /tmp/fresh.jsonl` was a usage error, and the
// launch in this directory went on appending to the log every earlier run had
// written -- test chatter included, because a session is every run in a
// directory and every request is the whole log.
#[rstest]
fn given_the_surface_and_a_named_session_when_planned_then_it_writes_that_file() {
    assert_eq!(
        plan(&["pearl", "--session", "/tmp/new.jsonl"]).unwrap(),
        Plan::Surface {
            session: "/tmp/new.jsonl".into(),
            resumed: true
        }
    );
}

// Coming back to a directory carries on with it. Starting over is the thing
// the human asks for, and the flag is global for the same reason `--session`
// is: both entry points carry a conversation, and only one of them meaning it
// would put the wish in the wrong program.
#[rstest]
fn given_a_fresh_flag_when_the_surface_is_planned_then_the_launch_begins_again() {
    assert_eq!(
        plan(&["pearl", "--fresh"]).unwrap(),
        Plan::Surface {
            session: "pearl-session.jsonl".into(),
            resumed: false
        }
    );
}

#[rstest]
fn given_a_fresh_flag_when_a_run_is_planned_then_the_launch_begins_again() {
    let drawn = plan(&["pearl", "run", "fix", "--fresh"]).unwrap();
    let Plan::Run { resumed, .. } = drawn else {
        panic!("a run was not planned: {drawn:?}");
    };
    assert!(!resumed);
}
