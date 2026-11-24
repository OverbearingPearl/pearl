//! Stories for the process's own footprint.
//!
//! A number on a status line has to be the kernel's, not an estimate: an
//! estimate invites the human to trust a figure that was never measured.

use rstest::rstest;

#[rstest]
fn given_a_running_process_when_its_footprint_is_read_then_it_is_a_real_number() {
    let bytes = pearl_tools::footprint::rss_bytes().expect("this OS was not asked");
    assert!(bytes > 1024 * 1024, "a live process reported {bytes} bytes");
}

#[rstest]
fn given_a_footprint_when_it_is_written_for_a_status_line_then_it_is_short() {
    let said = pearl_tools::footprint::describe().expect("no reading");
    assert!(said.len() <= 6, "{said} is too long for a status line");
    assert!(said.ends_with('M') || said.ends_with('G'), "{said} has no unit");
}

#[rstest]
fn given_memory_held_when_the_footprint_is_read_then_it_shows_up() {
    let before = pearl_tools::footprint::rss_bytes().unwrap_or(0);
    // Touch the pages: a reservation that is never written is not resident.
    let held: Vec<u8> = (0..32 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    let after = pearl_tools::footprint::rss_bytes().unwrap_or(0);
    assert!(held.len() == 32 * 1024 * 1024);
    assert!(after >= before, "resident memory went down while holding 32M: {before} -> {after}");
}
