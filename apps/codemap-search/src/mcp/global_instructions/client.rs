//! Select the global instruction file actually read by a supported MCP client.

use std::env;
use std::path::PathBuf;

use super::registry::read_document;

pub(super) enum Client {
    Codex,
    ClaudeCode,
    Pi,
    OpenCode,
}

impl Client {
    pub(super) fn identify(name: &str) -> Option<Self> {
        match name {
            "codex-mcp-client" => Some(Self::Codex),
            "claude-code" => Some(Self::ClaudeCode),
            "pi" => Some(Self::Pi),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }

    pub(super) fn instruction_path(&self, version: Option<&str>) -> Result<PathBuf, String> {
        let home = user_home_directory()?;
        match self {
            Self::Codex => {
                let directory = environment_directory("CODEX_HOME", home.join(".codex"))?;
                let override_path = directory.join("AGENTS.override.md");
                if read_document(&override_path)?.is_some_and(|text| !text.trim().is_empty()) {
                    return Ok(override_path);
                }
                Ok(directory.join("AGENTS.md"))
            }
            Self::ClaudeCode => Ok(environment_directory(
                "CLAUDE_CONFIG_DIR",
                home.join(".claude"),
            )?
            .join("CLAUDE.md")),
            Self::Pi => {
                let directory =
                    environment_directory("PI_CODING_AGENT_DIR", home.join(".pi/agent"))?;
                for name in [
                    "AGENTS.override.md",
                    "AGENTS.md",
                    "AGENTS.MD",
                    "CLAUDE.md",
                    "CLAUDE.MD",
                ] {
                    let path = directory.join(name);
                    if path.is_file() && read_document(&path)?.is_some() {
                        return Ok(path);
                    }
                }
                Ok(directory.join("AGENTS.md"))
            }
            Self::OpenCode => {
                let default_directory =
                    environment_directory("XDG_CONFIG_HOME", home.join(".config"))?
                        .join("opencode");
                let path = environment_directory("OPENCODE_CONFIG_DIR", default_directory)?
                    .join("AGENTS.md");
                if read_document(&path)?.is_some() {
                    return Ok(path);
                }
                let major_version =
                    version.and_then(|value| value.split('.').next()?.parse::<u64>().ok());
                if major_version.is_some_and(|major| major >= 2) {
                    return Ok(path);
                }
                // Creating AGENTS.md must not hide the existing Claude fallback in v1.
                let claude_path = home.join(".claude/CLAUDE.md");
                let is_fallback_disabled = is_environment_enabled("OPENCODE_DISABLE_CLAUDE_CODE")
                    || is_environment_enabled("OPENCODE_DISABLE_CLAUDE_CODE_PROMPT");
                if !is_fallback_disabled && read_document(&claude_path)?.is_some() {
                    match major_version {
                        Some(0 | 1) => return Ok(claude_path),
                        Some(_) => {},
                        None => return Err("OpenCode version is missing or invalid; cannot safely select its global instruction file".into()),
                    }
                }
                Ok(path)
            }
        }
    }
}

fn user_home_directory() -> Result<PathBuf, String> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let path = env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("cannot resolve the user home directory from {variable}"))?;
    require_absolute_directory(variable, path)
}

fn environment_directory(variable: &str, fallback: PathBuf) -> Result<PathBuf, String> {
    let path = env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(fallback);
    require_absolute_directory(variable, path)
}

fn require_absolute_directory(variable: &str, path: PathBuf) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{variable} must point to an absolute directory"));
    }
    Ok(path)
}

fn is_environment_enabled(variable: &str) -> bool {
    env::var(variable).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}
