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
    assert_eq!(plan(&["pearl"]).unwrap(), Plan::Surface);
}

#[rstest]
fn given_the_run_subcommand_when_planned_then_the_task_is_taken_with_the_defaults() {
    let drawn = plan(&["pearl", "run", "fix the tests"]).unwrap();
    let Plan::Run { task, model, session } = drawn else {
        panic!("a run was not planned: {drawn:?}");
    };
    assert_eq!(task, "fix the tests");
    assert_eq!(model, "deepseek-chat");
    assert_eq!(session, PathBuf::from("pearl-session.jsonl"));
}

#[rstest]
fn given_the_run_subcommand_with_flags_when_planned_then_the_flags_win() {
    let drawn = plan(&["pearl", "run", "fix", "--model", "qwen", "--session", "s.jsonl"]).unwrap();
    assert_eq!(drawn, Plan::Run { task: "fix".into(), model: "qwen".into(), session: "s.jsonl".into() });
}

#[rstest]
fn given_an_unknown_subcommand_when_planned_then_it_is_an_error() {
    assert!(plan(&["pearl", "sing"]).is_err());
}
