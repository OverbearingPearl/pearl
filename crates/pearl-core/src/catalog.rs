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
    /// The endpoint is normalised on the way in, so a row cannot carry a
    /// second `/chat/completions` the provider would append to.
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self { endpoint: normalize_endpoint(&endpoint.into()), model: model.into() }
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

/// The address as the wire wants it: no trailing slash, and no
/// `/chat/completions` on the end -- the provider appends that itself, and an
/// address that already carries it would ask for
/// `/chat/completions/chat/completions`. A human who copies the URL out of a
/// quickstart copies the whole thing, so this is a normalisation rather than
/// a complaint.
pub fn normalize_endpoint(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix("/chat/completions").unwrap_or(url);
    url.trim_end_matches('/').to_string()
}

/// The path of an endpoint URL, from the first `/` after the host.
fn path_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    match after.find('/') {
        Some(i) => after[i..].to_string(),
        None => String::new(),
    }
}

/// Why this address cannot be spoken to, when its path names somebody
/// else's API. `None` means the path is one the compatibility layer serves --
/// or empty, which is the ordinary case, because the provider appends
/// `/chat/completions` to whatever base it is given.
///
/// The point is to say so at the moment the address is typed, in one
/// sentence that also names the address that would have worked. A 404 four
/// turns later reads as pearl being broken.
pub fn protocol_complaint(url: &str) -> Option<String> {
    let base = normalize_endpoint(url);
    let path = path_of(&base).to_lowercase();
    if path.is_empty() {
        return None;
    }
    if path.ends_with("/messages") {
        let to = base.strip_suffix("/messages").unwrap_or(&base).to_string();
        let to = if to.is_empty() { String::from("<host>/v1") } else { to };
        return Some(format!(
            "that path is Anthropic's messages API; pearl appends /chat/completions, \
             so for Anthropic's own models the base is https://api.anthropic.com/v1 \
             (this host's would be {to})"
        ));
    }
    if path.contains(":generatecontent") {
        let origin = origin_of(&base);
        return Some(format!(
            "that path is Google's own API; for Gemini use the compatibility base \
             {origin}/v1beta/openai"
        ));
    }
    if path.contains("/api/chat") || path.contains("/api/generate") {
        return Some(format!(
            "that path is Ollama's native API; Ollama serves the compatible one at \
             {}/v1",
            origin_of(&base)
        ));
    }
    if path.ends_with("/completions") {
        let to = base.strip_suffix("/completions").unwrap_or(&base).to_string();
        let to = if to.is_empty() { String::from("<host>/v1") } else { to };
        return Some(format!(
            "that path is the legacy completions API; pearl appends /chat/completions, \
             so the base to give is {to}"
        ));
    }
    None
}

/// The scheme and host of an endpoint, with no path at all.
fn origin_of(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (format!("{s}://"), r),
        None => (String::new(), url),
    };
    let host = rest.split('/').next().unwrap_or(rest);
    format!("{scheme}{host}")
}

/// The host of an endpoint URL, without scheme, port or path.
pub fn host_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let host = after.split('/').next().unwrap_or(after);
    host.split(':').next().unwrap_or(host).to_string()
}

/// The houses whose first label is a word rather than their own name. The
/// list is short because each entry is a claim about somebody else's machine:
/// `open.bigmodel.cn` is Zhipu, `x.ai` is xAI, and the Generative Language
/// host is Google's.
const ODD_HOUSE: &[(&str, &str)] = &[
    ("open.bigmodel.cn", "bigmodel"),
    ("x.ai", "xai"),
    ("generativelanguage.googleapis.com", "gemini"),
];

/// The addresses the major houses publish for the compatibility layer. A row
/// here is a promise that requests to it work, so it holds only bases the
/// house itself documents -- and the launch endpoint's own house is left out,
/// because the launch endpoint already has a row.
pub const WELL_KNOWN: &[&str] = &[
    "https://api.openai.com/v1",
    "https://api.anthropic.com/v1",
    "https://dashscope.aliyuncs.com/compatible-mode/v1",
    "https://open.bigmodel.cn/api/paas/v4",
    "https://api.moonshot.cn/v1",
    "https://api.x.ai/v1",
    "https://api.groq.com/openai/v1",
    "https://openrouter.ai/api/v1",
    "https://generativelanguage.googleapis.com/v1beta/openai",
];

/// The house an address belongs to, or `None` when the host names a machine
/// rather than a company -- `localhost` and an IP are places, not houses.
pub fn house_of(url: &str) -> Option<String> {
    let host = host_of(url);
    if names_no_house(&host) { None } else { Some(provider_name(&host)) }
}

/// The house a machine belongs to: `api.deepseek.com` is `deepseek`.
pub fn provider_name(machine: &str) -> String {
    let host = machine.split(':').next().unwrap_or(machine);
    let trimmed = host.strip_prefix("api.").unwrap_or(host);
    if let Some((_, house)) = ODD_HOUSE.iter().find(|(m, _)| *m == trimmed) {
        return (*house).to_string();
    }
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
