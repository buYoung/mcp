//! Client-specific delivery limits. Server byte budgets remain independent.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientOutputConfig {
    pub claude_max_result_chars: Option<usize>,
    pub codex_output_token_limit: Option<usize>,
}

impl ClientOutputConfig {
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
