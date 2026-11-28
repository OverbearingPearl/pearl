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
        Self {
            endpoint: normalize_endpoint(&endpoint.into()),
            model: model.into(),
        }
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

/// The address as the wire wants it: the base the config stores, plus the
/// one path the provider appends. A surface shows this whole URL, because the
/// human is choosing where a request goes and `/chat/completions` is where it
/// goes; the config keeps the base, because the provider appends that path
/// itself.
pub fn wire_url(base: &str) -> String {
    let base = normalize_endpoint(base);
    if base.is_empty() {
        String::new()
    } else {
        format!("{base}/chat/completions")
    }
}

/// The host a person wrote, without the port it may carry.
fn bare_host(host: &str) -> &str {
    match host.rfind(':') {
        Some(i) if host[i + 1..].chars().all(|c| c.is_ascii_digit()) => &host[..i],
        _ => host,
    }
}

/// Whether a host is this machine or the network around it. A local address is
/// where a server the human just started lives, and it usually speaks plain
/// http; a house on the network is spoken to over TLS, because a key travels
/// with the request.
pub fn names_this_machine(host: &str) -> bool {
    use std::net::IpAddr;
    let host = bare_host(host.trim()).trim_matches(|c| c == '[' || c == ']');
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        Ok(IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => false,
    }
}

/// What a typed address resolved to: the base the config stores, and the one
/// thing the human may not have written -- a path that named another API and
/// was read as chat instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed {
    pub endpoint: String,
    pub note: Option<String>,
}

/// Read an address the way a person writes one and keep the base. A whole
/// request URL is what a quickstart shows, so its `/chat/completions` is
/// removed; a scheme is supplied by the host (http here, https there); and a
/// path that names OpenAI's responses API or Anthropic's messages API is read
/// as the chat base instead, with the note saying so. A refusal names the
/// address that would work.
pub fn complete_endpoint(typed: &str) -> Result<Completed, String> {
    let text = typed.trim();
    if text.is_empty() {
        return Err(String::from("an endpoint needs an address"));
    }
    let (scheme, rest) = match text.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => (String::new(), text),
    };
    if !scheme.is_empty() && scheme != "http" && scheme != "https" {
        return Err(format!(
            "pearl speaks https to a house and http to this machine, not {scheme}://; \
             write https://{rest}"
        ));
    }
    let host = rest.split('/').next().unwrap_or(rest);
    let bare = bare_host(host).trim_matches(|c| c == '[' || c == ']');
    if bare.is_empty()
        || (!bare.contains('.')
            && bare != "localhost"
            && bare.parse::<std::net::IpAddr>().is_err())
    {
        return Err(format!(
            "{host:?} is not a host; an address looks like https://openrouter.ai/api/v1"
        ));
    }
    let local = names_this_machine(host);
    let scheme = match scheme.as_str() {
        "" if local => "http",
        "" => "https",
        other => other,
    };
    if scheme == "http" && !local {
        return Err(format!(
            "pearl sends the key with the request, so a house is spoken to over https; \
             write https://{rest}"
        ));
    }
    let mut path = path_of(rest).trim_end_matches('/').to_string();
    let lower = path.to_ascii_lowercase();
    let mut note = None;
    if lower.ends_with("/chat/completions") {
        path.truncate(path.len() - "/chat/completions".len());
    } else if let Some(suffix) = ["/responses", "/messages"]
        .into_iter()
        .find(|suffix| lower.ends_with(suffix))
    {
        path.truncate(path.len() - suffix.len());
        note = Some(format!(
            "pearl speaks openai-chat only, so {suffix} was read as /chat/completions"
        ));
    }
    Ok(Completed {
        endpoint: normalize_endpoint(&format!("{scheme}://{host}{path}")),
        note,
    })
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
/// messages later reads as pearl being broken.
pub fn protocol_complaint(url: &str) -> Option<String> {
    let base = normalize_endpoint(url);
    let path = path_of(&base).to_lowercase();
    if path.is_empty() {
        return None;
    }
    if path.ends_with("/messages") {
        let to = base.strip_suffix("/messages").unwrap_or(&base).to_string();
        let to = if to.is_empty() {
            String::from("<host>/v1")
        } else {
            to
        };
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
        let to = base
            .strip_suffix("/completions")
            .unwrap_or(&base)
            .to_string();
        let to = if to.is_empty() {
            String::from("<host>/v1")
        } else {
            to
        };
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
/// `open.bigmodel.cn` is Zhipu, `x.ai` is xAI, the Generative Language host is
/// Google's, and `spark-api-open` is iFlytek's Spark.
const ODD_HOUSE: &[(&str, &str)] = &[
    ("open.bigmodel.cn", "bigmodel"),
    ("x.ai", "xai"),
    ("generativelanguage.googleapis.com", "gemini"),
    ("spark-api-open.xf-yun.com", "spark"),
    ("dashscope-intl.aliyuncs.com", "dashscope"),
    ("dashscope-us.aliyuncs.com", "dashscope"),
];

/// The domains whose subdomains are deployments rather than houses. A
/// dedicated Bailian address puts its own id where the house usually sits --
/// `llm-…`, `token-plan` -- while the vendor is the domain it lives under, and
/// the thinking fields belong to the gateway, not to the deployment.
const HOUSE_SUFFIX: &[(&str, &str)] = &[
    ("maas.aliyuncs.com", "dashscope"),
    ("maas.qwencloudapi.com", "dashscope"),
    ("maas.qianwenaiapi.com", "dashscope"),
];

/// The house a domain names, matched at a dot boundary so a host that merely
/// shares the vendor's letters is not claimed.
fn suffix_house(host: &str) -> Option<&'static str> {
    HOUSE_SUFFIX.iter().find_map(|(suffix, house)| {
        host.strip_suffix(suffix)
            .filter(|head| head.is_empty() || head.ends_with('.'))
            .map(|_| *house)
    })
}

/// The bases the houses publish for the OpenAI-compatible layer, in the order
/// a row is offered them. Each one is a promise that a request sent there will
/// be answered, so an entry goes in only when the house's own documentation
/// names it and the address answers a request without a key (401, 403 or 429 --
/// "who are you", not "no such path"). A base that the house documents only as
/// a native protocol is not in this list: it is refused by `protocol_complaint`
/// and pointing a row at it would be a lie.
///
/// The aggregators are here because a person who pays one of them can reach
/// several houses through a single key, and that is the whole of what they buy.
pub const WELL_KNOWN: &[&str] = &[
    "https://api.openai.com/v1",
    "https://api.anthropic.com/v1",
    "https://api.deepseek.com",
    "https://generativelanguage.googleapis.com/v1beta/openai",
    "https://api.x.ai/v1",
    "https://api.groq.com/openai/v1",
    "https://api.mistral.ai/v1",
    "https://openrouter.ai/api/v1",
    "https://api.siliconflow.cn/v1",
    "https://api.ofox.io/v1",
    "https://dashscope.aliyuncs.com/compatible-mode/v1",
    "https://open.bigmodel.cn/api/paas/v4",
    "https://api.moonshot.cn/v1",
    "https://api.minimaxi.com/v1",
    "https://api.xiaomimimo.com/v1",
    "https://ark.cn-beijing.volces.com/api/v3",
    "https://qianfan.baidubce.com/v2",
    "https://api.hunyuan.cloud.tencent.com/v1",
    "https://tokenhub.tencentmaas.com/v1",
    "https://spark-api-open.xf-yun.com/v1",
];

/// The house an address belongs to, or `None` when the host names a machine
/// rather than a company -- `localhost` and an IP are places, not houses.
pub fn house_of(url: &str) -> Option<String> {
    let host = host_of(url);
    if names_no_house(&host) {
        None
    } else {
        Some(provider_name(&host))
    }
}

/// The base an address for this host speaks to, when the house publishes one.
///
/// A host a human already holds a key for is a machine they can use, but the
/// path after it is the vendor's, not theirs: a Bailian deployment lives at
/// `/compatible-mode/v1`, and remembering that is not the human's job. Only
/// hosts under a house this catalog can name are completed; a host under no
/// known house would make the rest a guess.
pub fn base_for_host(host: &str) -> Option<String> {
    let host = host.trim().trim_end_matches('/');
    if host.is_empty() || names_no_house(host) {
        return None;
    }
    if let Some(base) = WELL_KNOWN.iter().find(|base| host_of(base) == host) {
        return Some((*base).to_string());
    }
    // Only a deployment under a vendor's own domain is completed: the host is
    // the machine and the vendor's published base supplies the path. An odd
    // house with no exact published host is left alone -- rewriting `x.ai`
    // into `api.x.ai` would be a guess about somebody else's DNS.
    let house = suffix_house(host)?;
    let published = WELL_KNOWN
        .iter()
        .find(|base| provider_name(&host_of(base)) == house)?;
    let rest = published.strip_prefix("https://")?;
    let path = &rest[rest.find('/')?..];
    Some(format!("https://{host}{path}"))
}

/// The house a machine belongs to: `api.deepseek.com` is `deepseek`.
pub fn provider_name(machine: &str) -> String {
    let host = machine.split(':').next().unwrap_or(machine);
    let trimmed = host.strip_prefix("api.").unwrap_or(host);
    if let Some((_, house)) = ODD_HOUSE.iter().find(|(m, _)| *m == trimmed) {
        return (*house).to_string();
    }
    if let Some(house) = suffix_house(trimmed) {
        return house.to_string();
    }
    let first = trimmed.split('.').next().unwrap_or(trimmed);
    if first.is_empty() {
        host.to_string()
    } else {
        first.to_string()
    }
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
    if url.starts_with("https") {
        String::from("443")
    } else {
        String::from("80")
    }
}
