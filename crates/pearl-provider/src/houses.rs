//! houses --- the endpoint and the key, found at the moment of the request.
//!
//! A model choice is an endpoint and an id, and the human may change it while
//! the work runs. A client built once at launch cannot follow that: it would
//! send the new model's name to the old model's server. So the loop holds one
//! of these instead, and each request asks where it is going and who is paying
//! before it opens a connection. A local server that wants no key is answered
//! the same way -- no key is a valid answer, not a failure to start.

use std::path::PathBuf;

use crate::auth::{self, Credential};
use crate::openai::OpenAiProvider;
use crate::provider::{Provider, ProviderError};
use crate::types::{Completion, CompletionRequest};

/// Routes each request to the endpoint in force, resolving its key at use.
pub struct Houses<F> {
    launch: String,
    choose: F,
    authinfo: Option<PathBuf>,
}

impl<F: Fn() -> String> Houses<F> {
    /// `launch` is where a candidate that names no endpoint goes; `choose`
    /// reads the endpoint in force, and may return empty to mean that one.
    pub fn new(launch: impl Into<String>, choose: F) -> Self {
        Self { launch: launch.into(), choose, authinfo: auth::default_path() }
    }

    /// Read credentials from somewhere other than `~/.authinfo`.
    pub fn with_authinfo(mut self, path: Option<PathBuf>) -> Self {
        self.authinfo = path;
        self
    }

    /// The base URL this request will be sent to.
    pub fn endpoint(&self) -> String {
        let chosen = (self.choose)();
        if chosen.is_empty() { self.launch.clone() } else { chosen }
    }

    /// The key for that endpoint, or none. The house's own env var wins over
    /// the file: a key set for this run is a deliberate override of a file
    /// that serves every tool on the machine. No other house's key is
    /// borrowed -- a wrong key is worse than no key, because it is sent.
    pub fn credential(&self) -> Option<Credential> {
        let host = auth::host_of(&self.endpoint());
        let var = format!("{}_API_KEY", auth::provider_name(&host).to_uppercase());
        if let Some(key) = std::env::var(&var).ok().filter(|k| !k.is_empty()) {
            return Some(Credential { key, provider: auth::provider_name(&host) });
        }
        let path = self.authinfo.clone()?;
        auth::resolve(&host, None, &path)
    }
}

impl<F: Fn() -> String + Sync> Provider for Houses<F> {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let key = self.credential().map(|c| c.key).unwrap_or_default();
        OpenAiProvider::new(self.endpoint(), key).complete(request).await
    }
}
