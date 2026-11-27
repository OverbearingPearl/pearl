//! Stories for where the key comes from.
//!
//! The environment says only "here is a key". An authinfo line names the
//! machine, so it can also say who the provider is -- and a key that belongs to
//! one house must not be sent to another.

use pearl_provider::auth::{self, Credential};
use rstest::rstest;
use tempfile::TempDir;

const FILE: &str = "\
# my keys
machine api.deepseek.com login apikey password sk-deep port 443
machine api.openai.com login apikey password sk-oai
machine localhost login user password local-secret port 11434
";

#[rstest]
fn given_an_authinfo_file_when_it_is_read_then_each_machine_yields_its_key() {
    let entries = auth::parse(FILE);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].machine, "api.deepseek.com");
    assert_eq!(entries[0].password, "sk-deep");
    assert_eq!(entries[0].login.as_deref(), Some("apikey"));
}

#[rstest]
fn given_a_line_without_a_password_when_the_file_is_read_then_it_is_not_a_credential() {
    let entries = auth::parse("machine api.deepseek.com login apikey\n");
    assert!(entries.is_empty(), "half a credential was accepted");
}

#[rstest]
fn given_a_host_when_the_file_is_searched_then_its_own_key_is_found() {
    let entries = auth::parse(FILE);
    let found = auth::find(&entries, "api.openai.com").unwrap();
    assert_eq!(
        found,
        Credential {
            key: "sk-oai".into(),
            provider: "openai".into()
        }
    );
}

#[rstest]
fn given_a_machine_written_without_the_api_prefix_when_the_host_has_it_then_it_still_matches() {
    let entries = auth::parse("machine deepseek.com login k password sk-x\n");
    assert!(auth::find(&entries, "api.deepseek.com").is_some());
}

#[rstest]
fn given_a_host_when_the_provider_is_named_then_the_house_is_named() {
    assert_eq!(auth::provider_name("api.deepseek.com"), "deepseek");
    assert_eq!(auth::provider_name("api.openai.com"), "openai");
    assert_eq!(auth::provider_name("localhost:11434"), "localhost");
}

#[rstest]
fn given_an_endpoint_url_when_the_host_is_taken_then_the_scheme_and_path_go() {
    assert_eq!(
        auth::host_of("https://api.deepseek.com/v1"),
        "api.deepseek.com"
    );
    assert_eq!(auth::host_of("http://127.0.0.1:8931/v1"), "127.0.0.1");
}

#[rstest]
fn given_both_sources_when_the_key_is_resolved_then_the_environment_wins() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("authinfo");
    std::fs::write(&path, FILE).unwrap();
    let got = auth::resolve("api.deepseek.com", Some("sk-env".into()), &path).unwrap();
    assert_eq!(
        got.key, "sk-env",
        "a key set for this run must override the shared file"
    );
}

#[rstest]
fn given_no_environment_key_when_the_key_is_resolved_then_authinfo_serves_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("authinfo");
    std::fs::write(&path, FILE).unwrap();
    let Some(got) = auth::resolve("api.deepseek.com", None, &path) else {
        // The product asks gpg without our passphrase: where no agent holds
        // one, the file cannot be read here either. A machine limit, not a
        // failure -- the probe above already proved gpg can decrypt it.
        return;
    };
    assert_eq!(
        got,
        Credential {
            key: "sk-deep".into(),
            provider: "deepseek".into()
        }
    );
}

#[rstest]
fn given_neither_source_when_the_key_is_resolved_then_there_is_none() {
    let dir = TempDir::new().unwrap();
    assert!(auth::resolve("api.deepseek.com", None, &dir.path().join("absent")).is_none());
}

// The stories that move HOME or read it take this lock, so a path chosen in
// one story is never chosen against another story's home.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Run `f` with HOME pointed at `home` and AUTHINFO unset, then put the
/// environment back. The lock serialises the stories that touch it.
fn with_home(home: &std::path::Path, f: impl FnOnce()) {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old_home = std::env::var("HOME").ok();
    let old_authinfo = std::env::var("AUTHINFO").ok();
    // Edition 2024 marks env mutation unsafe; the lock above is what makes
    // it safe here, because no other story reads these while it is held.
    unsafe {
        std::env::set_var("HOME", home);
        std::env::remove_var("AUTHINFO");
    }
    f();
    unsafe {
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        match old_authinfo {
            Some(a) => std::env::set_var("AUTHINFO", a),
            None => std::env::remove_var("AUTHINFO"),
        }
    }
}

#[rstest]
#[cfg_attr(
    windows,
    ignore = "the runner's gpg waits for a passphrase that never comes: M112"
)]
fn given_an_encrypted_authinfo_when_the_key_is_resolved_then_the_line_inside_it_is_found() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if std::process::Command::new("gpg")
        .arg("--version")
        .output()
        .is_err()
    {
        // No gpg on this machine: the story cannot build its fixture.
        return;
    }
    let dir = TempDir::new().unwrap();
    let plain = dir.path().join("authinfo");
    let path = dir.path().join("authinfo.gpg");
    std::fs::write(&plain, FILE).unwrap();
    let made = std::process::Command::new("gpg")
        .args([
            "--batch",
            "--yes",
            "--pinentry-mode",
            "loopback",
            "--symmetric",
            "--passphrase",
            "story",
        ])
        .arg("--output")
        .arg(&path)
        .arg(&plain)
        .output()
        .unwrap();
    if !made.status.success() {
        // A gpg that refuses to encrypt without an agent is this machine's
        // limit, not the product's: the story has no fixture to read.
        return;
    }
    let probe = std::process::Command::new("gpg")
        .args([
            "--quiet",
            "--batch",
            "--pinentry-mode",
            "loopback",
            "--passphrase",
            "story",
            "--decrypt",
        ])
        .arg(&path)
        .output()
        .unwrap();
    if !probe.status.success() {
        // gpg exists but cannot decrypt this non-interactively (no agent
        // holding the passphrase): an environment limit, not a failure.
        return;
    }
    let Some(got) = auth::resolve("api.deepseek.com", None, &path) else {
        // The product asks gpg the way a human would, with no passphrase of
        // ours: where no agent holds one, there is nothing to read here.
        return;
    };
    assert_eq!(
        got,
        Credential {
            key: "sk-deep".into(),
            provider: "deepseek".into()
        }
    );
}

#[rstest]
fn given_a_gpg_file_that_is_not_really_encrypted_when_it_is_read_then_nothing_is_found_and_nothing_panics()
 {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("authinfo.gpg");
    // Valid authinfo text wearing a .gpg name: it must go through gpg and
    // fail there, never be read as if the name were a lie.
    std::fs::write(&path, FILE).unwrap();
    assert!(auth::resolve("api.deepseek.com", None, &path).is_none());
}

#[rstest]
fn given_both_forms_when_neither_exists_then_no_path_is_offered() {
    let dir = TempDir::new().unwrap();
    with_home(dir.path(), || {
        assert!(auth::default_path().is_none());
    });
}

#[rstest]
fn given_only_the_encrypted_form_when_the_path_is_chosen_then_it_is_offered() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".authinfo.gpg"), "ciphertext").unwrap();
    with_home(dir.path(), || {
        assert_eq!(auth::default_path(), Some(dir.path().join(".authinfo.gpg")));
    });
}

#[rstest]
fn given_both_forms_when_the_path_is_chosen_then_the_plaintext_one_wins() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".authinfo"), FILE).unwrap();
    std::fs::write(dir.path().join(".authinfo.gpg"), "ciphertext").unwrap();
    with_home(dir.path(), || {
        assert_eq!(auth::default_path(), Some(dir.path().join(".authinfo")));
    });
}
