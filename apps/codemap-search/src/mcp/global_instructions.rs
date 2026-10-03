//! Opt-in global instruction synchronization, driven by MCP identity and config reloads.

mod client;
mod registry;

use client::Client;
use serde_json::Value;

#[derive(Default)]
pub(super) struct GlobalInstructions {
    client: Option<Client>,
    client_version: Option<String>,
    has_initialized: bool,
    is_enabled: Option<bool>,
}

impl GlobalInstructions {
    pub(super) fn initialize(&mut self, params: Option<&Value>, is_enabled: bool) {
        self.client = params
            .and_then(|params| params.pointer("/clientInfo/name"))
            .and_then(Value::as_str)
            .and_then(Client::identify);
        self.client_version = params
            .and_then(|params| params.pointer("/clientInfo/version"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.has_initialized = true;
        self.is_enabled = None;
        self.synchronize(is_enabled);
    }

    pub(super) fn synchronize(&mut self, is_enabled: bool) {
        if !self.has_initialized || self.is_enabled == Some(is_enabled) {
            return;
        }
        // Retry failures on the next initialize or toggle, without repeated request warnings.
        self.is_enabled = Some(is_enabled);
        if let Err(error) = self.synchronize_files(is_enabled) {
            tracing::warn!("global instruction sync skipped: {error}");
        }
    }

    fn synchronize_files(&self, is_enabled: bool) -> Result<(), String> {
        let path = if is_enabled {
            let client = self
                .client
                .as_ref()
                .ok_or("unsupported or missing MCP clientInfo.name")?;
            Some(client.instruction_path(self.client_version.as_deref())?)
        } else {
            None
        };
        registry::synchronize(&crate::config::global_dir(), path.as_deref())
    }
}
