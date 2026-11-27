//! Stories for what a long run holds in memory.
//!
//! Every shell op keeps a before-image so it can be taken back, and the human
//! watches `mem` on the status line while a session runs. The promise is both
//! halves at once: the process does not grow with the number of commands, and
//! the oldest op is still revertible to the exact bytes it started from.

use pearl_agent::{Policy, ToolRegistry};
use rstest::rstest;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

const MIB: u64 = 1024 * 1024;

#[rstest]
#[tokio::test]
async fn given_a_large_work_file_when_many_shell_ops_run_then_the_process_holds_no_copy_of_each() {
    let dir = TempDir::new().unwrap();
    // A workspace with one substantial file in it: an asset, a fixture, a data
    // file. Nothing here is build output, so a baseline keeps every byte.
    fs::write(dir.path().join("big.bin"), vec![b'x'; 8 * MIB as usize]).unwrap();
    fs::write(dir.path().join("work.txt"), "original").unwrap();
    let mut tools = ToolRegistry::new(dir.path(), Policy::WorkspaceWrite);

    let before = pearl_tools::footprint::rss_bytes().expect("the OS reports no footprint");
    for i in 0..12 {
        let out = tools
            .call("bash", json!({"command": format!("echo {i} >> work.txt")}))
            .await;
        assert!(!out.is_error, "the command did not run: {}", out.content);
    }
    let after = pearl_tools::footprint::rss_bytes().expect("the OS reports no footprint");

    // Twelve ops over an 8 MiB file is 96 MiB if each keeps its own copy.
    assert!(
        after.saturating_sub(before) < 32 * MIB,
        "the ledger is holding a copy per op: mem grew {} MiB over 12 commands",
        after.saturating_sub(before) / MIB
    );

    // And the promise the memory was paying for still holds: the FIRST op
    // goes back to the bytes the human had, twelve commands later.
    let touched = tools.revert_bash(0).unwrap();
    assert!(touched >= 1, "the oldest op could not be taken back");
    assert_eq!(
        fs::read_to_string(dir.path().join("work.txt")).unwrap(),
        "original"
    );
    assert_eq!(
        fs::metadata(dir.path().join("big.bin")).unwrap().len(),
        8 * MIB
    );
}
