//! auth --- where the key comes from, resolved at use and never stored.
//!
//! Two sources, in order: the environment, then authinfo -- `~/.authinfo`,
//! or `~/.authinfo.gpg` decrypted by the user's own gpg. The environment
//! says only "here is a key"; an authinfo line names the machine it belongs to,
//! so it can also say who the provider is -- which is worth showing, because
//! `deepseek-chat` and `gpt-4o` are answers to different questions.

use std::path::{Path, PathBuf};

/// A key and the name of the house it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub key: String,
    pub provider: String,
}

/// One `machine … login … password …` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub machine: String,
    pub login: Option<String>,
    pub password: String,
}

/// Parse authinfo text. Comments and blank lines are skipped; a line without a
/// machine or without a password is not a credential and is ignored rather
/// than half-read.
pub fn parse(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let field = |name: &str| -> Option<String> {
            words
                .iter()
                .position(|w| *w == name)
                .and_then(|i| words.get(i + 1))
                .map(|s| s.to_string())
        };
        let (Some(machine), Some(password)) = (field("machine"), field("password")) else {
            continue;
        };
        out.push(Entry {
            machine,
            login: field("login"),
            password,
        });
    }
    out
}

/// The provider a machine name belongs to, read by the catalog's rule:
/// `api.deepseek.com` is deepseek, `localhost` is itself, and a deployment
/// under a vendor's domain belongs to that vendor. One derivation for the
/// header and the key, so the two cannot disagree.
pub fn provider_name(machine: &str) -> String {
    pearl_core::catalog::provider_name(machine)
}

/// The machines an authinfo file names, without their keys. A surface may
/// offer these as places a request can go -- the key is what proves the human
/// can reach them -- but the key itself never leaves this module.
pub fn machines(path: &Path) -> Vec<String> {
    let Some(text) = read_text(path) else {
        return Vec::new();
    };
    let mut hosts: Vec<String> = Vec::new();
    for entry in parse(&text) {
        if !hosts.contains(&entry.machine) {
            hosts.push(entry.machine);
        }
    }
    hosts
}

/// The host of an endpoint URL, without scheme, port or path.
pub fn host_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let host = after.split('/').next().unwrap_or(after);
    host.split(':').next().unwrap_or(host).to_string()
}

/// Find the credential for a host. An entry matches when its machine is the
/// host, or the host with an `api.` prefix -- people write both.
pub fn find(entries: &[Entry], host: &str) -> Option<Credential> {
    let wanted = host.strip_prefix("api.").unwrap_or(host);
    entries
        .iter()
        .find(|e| {
            let m = e.machine.as_str();
            let m = m.strip_prefix("api.").unwrap_or(m);
            m == wanted || e.machine == host
        })
        .map(|e| Credential {
            key: e.password.clone(),
            provider: provider_name(&e.machine),
        })
}

/// Where authinfo lives: `$AUTHINFO`, else `~/.authinfo` when it exists,
/// else `~/.authinfo.gpg` when that does. The encrypted form is read through
/// the user's own gpg, so the keys and the agent stay where they already are.
pub fn default_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AUTHINFO") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    let home = PathBuf::from(std::env::var("HOME").ok()?);
    let plain = home.join(".authinfo");
    if plain.exists() {
        return Some(plain);
    }
    let encrypted = home.join(".authinfo.gpg");
    if encrypted.exists() {
        return Some(encrypted);
    }
    None
}

/// The text of an authinfo file. A `.gpg` name means the bytes are
/// ciphertext and go through gpg; anything else is read as it stands.
fn read_text(path: &Path) -> Option<String> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gpg"))
    {
        decrypt(path)
    } else {
        std::fs::read_to_string(path).ok()
    }
}

/// Ask the user's gpg to decrypt, the way Emacs does. `--batch` makes gpg
/// fail rather than prompt when no agent can answer, and the wait is bounded
/// besides: a pinentry that appears anyway must not hold the surface forever.
/// A missing gpg, a refusal or a timeout is no credential -- never a fallback
/// to reading the ciphertext as text. The plaintext is returned, not logged.
fn decrypt(path: &Path) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let mut child = Command::new("gpg")
        .args(["--quiet", "--batch", "--decrypt"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Drain stdout on a thread so a file larger than the pipe buffer cannot
    // deadlock the wait below.
    let mut pipe = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        pipe.read_to_end(&mut buf).ok().map(|_| buf)
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break false,
        }
    };
    if !ok {
        let _ = child.kill();
    }
    let _ = child.wait();
    let bytes = reader.join().ok()??;
    if !ok {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Resolve the key for an endpoint at the moment it is needed. The environment
/// wins: a key set for this run is a deliberate override of a file that serves
/// every tool on the machine.
pub fn resolve(host: &str, env_key: Option<String>, authinfo: &Path) -> Option<Credential> {
    if let Some(key) = env_key.filter(|k| !k.is_empty()) {
        return Some(Credential {
            key,
            provider: provider_name(host),
        });
    }
    let text = read_text(authinfo)?;
    find(&parse(&text), host)
}
/// The models a house is known to serve, for the cycle key. An unknown house
/// returns nothing: guessing a model name for someone's local endpoint would
/// replace a working choice with a broken one.
pub fn models_for(provider: &str) -> Vec<String> {
    let names: &[&str] = match provider {
        "deepseek" => &["deepseek-chat", "deepseek-reasoner"],
        "openai" => &["gpt-4o", "gpt-4o-mini"],
        _ => &[],
    };
    names.iter().map(|s| s.to_string()).collect()
}
