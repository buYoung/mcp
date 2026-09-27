//! Client delivery limits: the largest result each coding agent passes to its model. The
//! smallest configured limit bounds every final response; `max_bytes` budgets apply earlier.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientOutputConfig {
    pub claude_max_result_chars: Option<usize>,
    pub codex_output_token_limit: Option<usize>,
    /// pi-mcp-adapter `settings.outputGuard.maxBytes`.
    pub pi_max_bytes: Option<usize>,
    /// opencode `tool_output.max_bytes`.
    pub opencode_max_bytes: Option<usize>,
}

impl ClientOutputConfig {
    /// The smallest configured client limit in UTF-8 bytes. Counting Claude characters as
    /// bytes never undercounts them; Codex tokens use a 3.5-byte estimate with JSON headroom.
    pub fn delivery_byte_cap(&self) -> Option<usize> {
        let codex_bytes = self
            .codex_output_token_limit
            .map(|tokens| tokens.saturating_mul(7) / 2);
        [
            self.claude_max_result_chars,
            codex_bytes,
            self.pi_max_bytes,
            self.opencode_max_bytes,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Render an explicit configuration fragment; never modify the client's settings.
    pub fn codex_config(&self, server_name: &str) -> Result<String, toml::ser::Error> {
        let Some(limit) = self.codex_output_token_limit else {
            return Ok("# output.client.codex_output_token_limit is not set.\n".into());
        };
        let mut tools = toml::Table::new();
        for tool in crate::tools::list_tools()["tools"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(name) = tool["name"].as_str() {
                let mut settings = toml::Table::new();
                settings.insert(
                    "output_token_limit".into(),
                    toml::Value::Integer(limit as i64),
                );
                tools.insert(name.into(), toml::Value::Table(settings));
            }
        }
        let mut server = toml::Table::new();
        server.insert("tools".into(), toml::Value::Table(tools));
        let mut servers = toml::Table::new();
        servers.insert(server_name.into(), toml::Value::Table(server));
        let mut config = toml::Table::new();
        config.insert("mcp_servers".into(), toml::Value::Table(servers));
        toml::to_string_pretty(&config)
    }
}
