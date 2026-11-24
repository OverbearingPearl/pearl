//! catalog --- what a model choice is made of.
//!
//! A candidate is an endpoint and a model id, and nothing else. The house is
//! read off the endpoint rather than typed beside it: a second field that has
//! to agree with the first is a field that will disagree with it. The key is
//! not here at all -- it belongs to whoever runs the machine, and pearl only
//! looks for it at the moment a request needs it.

/// One model the human may choose: where to ask, and what to ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The OpenAI-compatible base URL. Empty means the endpoint the process
    /// was launched against, which is what a config written before endpoints
    /// were choices still means.
    pub endpoint: String,
    pub model: String,
}

impl Candidate {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self { endpoint: endpoint.into(), model: model.into() }
    }

    /// The host without its port, which is what a credential is filed under.
    pub fn host(&self) -> String {
        host_of(&self.endpoint)
    }

    /// The house, read off the endpoint.
    pub fn provider(&self) -> String {
        provider_name(&self.host())
    }

    /// The row the human reads. Two servers on one machine are two rows, so a
    /// host that names no house carries its port: `localhost/qwen3` twice is
    /// a list nobody can choose from.
    pub fn label(&self) -> String {
        if self.endpoint.is_empty() {
            return self.model.clone();
        }
        let host = self.host();
        if names_no_house(&host) {
            format!("{}:{}/{}", host, port_of(&self.endpoint), self.model)
        } else {
            format!("{}/{}", self.provider(), self.model)
        }
    }
}

/// The host of an endpoint URL, without scheme, port or path.
pub fn host_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let host = after.split('/').next().unwrap_or(after);
    host.split(':').next().unwrap_or(host).to_string()
}

/// The house a machine belongs to: `api.deepseek.com` is `deepseek`.
pub fn provider_name(machine: &str) -> String {
    let host = machine.split(':').next().unwrap_or(machine);
    let trimmed = host.strip_prefix("api.").unwrap_or(host);
    let first = trimmed.split('.').next().unwrap_or(trimmed);
    if first.is_empty() { host.to_string() } else { first.to_string() }
}

/// Whether a host is an address rather than a house. `localhost`, a bare
/// machine name and any IP all say where, not who.
fn names_no_house(host: &str) -> bool {
    !host.contains('.') || host.parse::<std::net::IpAddr>().is_ok()
}

/// The port an endpoint speaks on, stated or implied by its scheme.
fn port_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let authority = after.split('/').next().unwrap_or(after);
    if let Some(p) = authority.rsplit_once(':').map(|(_, p)| p) {
        if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
            return p.to_string();
        }
    }
    if url.starts_with("https") { String::from("443") } else { String::from("80") }
}
