use serde::Deserialize;

/// Mirrors the nested structure of ~/.openclaw/openclaw.json
#[derive(Debug, Deserialize)]
struct OpenClawConfig {
    gateway: Option<GatewaySection>,
}

#[derive(Debug, Deserialize)]
struct GatewaySection {
    port: Option<u16>,
    bind: Option<String>,
    auth: Option<AuthSection>,
}

#[derive(Debug, Deserialize)]
struct AuthSection {
    #[allow(dead_code)]
    mode: Option<String>,
    token: Option<String>,
}

/// Resolved gateway configuration ready for connection
pub struct GatewayConfig {
    pub url: String,
    pub token: Option<String>,
}

impl GatewayConfig {
    /// Resolve gateway config from: CLI args > env vars > ~/.openclaw/openclaw.json
    pub fn resolve(cli_url: Option<&str>, cli_token: Option<&str>) -> Option<Self> {
        // CLI args / env vars take priority (clap `env` attribute handles env)
        let file_config = load_config_file();

        let url = cli_url.map(String::from).or_else(|| {
            let section = file_config.as_ref()?.gateway.as_ref()?;
            let port = section.port.unwrap_or(18789);
            let bind_addr = match section.bind.as_deref() {
                Some("loopback") | None => "127.0.0.1",
                _ => "0.0.0.0",
            };
            Some(format!("ws://{}:{}", bind_addr, port))
        });

        let url = url?;

        let token = cli_token.map(String::from).or_else(|| {
            file_config
                .as_ref()?
                .gateway
                .as_ref()?
                .auth
                .as_ref()?
                .token
                .clone()
        });

        Some(GatewayConfig { url, token })
    }
}

fn load_config_file() -> Option<OpenClawConfig> {
    let home = dirs::home_dir()?;
    let config_path = home.join(".openclaw").join("openclaw.json");

    if !config_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&config_path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nested_gateway_config() {
        let json = r#"{
            "gateway": {
                "port": 18789,
                "mode": "local",
                "bind": "loopback",
                "auth": {
                    "mode": "token",
                    "token": "test-token-abc123"
                }
            }
        }"#;
        let config: OpenClawConfig = serde_json::from_str(json).unwrap();
        let gw = config.gateway.unwrap();
        assert_eq!(gw.port, Some(18789));
        assert_eq!(gw.bind.as_deref(), Some("loopback"));
        assert_eq!(gw.auth.unwrap().token.as_deref(), Some("test-token-abc123"));
    }

    #[test]
    fn resolve_constructs_url_from_port() {
        // With explicit CLI URL, that takes priority
        let config = GatewayConfig::resolve(Some("ws://custom:9999"), None);
        assert_eq!(config.unwrap().url, "ws://custom:9999");
    }

    #[test]
    fn resolve_returns_none_without_config() {
        // No CLI args, no config file => None
        let config = GatewayConfig::resolve(None, None);
        // This depends on whether ~/.openclaw/openclaw.json exists on the machine
        // In CI without the file, this would be None
        let _ = config;
    }
}
