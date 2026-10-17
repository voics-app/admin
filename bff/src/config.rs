//! Environment configuration (P2), validated once at startup.

use thiserror::Error;

use crate::domain::admin_allowlist::AdminAllowlist;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Local,
    Production,
}

/// Cloudflare Access application the admin portal sits behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareAccess {
    pub team_domain: String,
    pub audience: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub environment: Environment,
    pub log_level: String,
    pub otel_endpoint: Option<String>,
    pub internal_service_token: String,
    pub kafka_brokers: Vec<String>,
    pub database_url: String,
    pub api_internal_url: String,
    pub gateway_internal_url: String,
    pub victoriametrics_url: String,
    pub etcd_endpoints: Vec<String>,
    pub allowlist: AdminAllowlist,
    pub cloudflare_access: Option<CloudflareAccess>,
}

impl Config {
    /// The `X-Dev-Admin-Email` shortcut exists only for local development.
    pub fn accepts_dev_header(&self) -> bool {
        self.environment == Environment::Local
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid configuration:\n  - {}", .0.join("\n  - "))]
pub struct ConfigError(pub Vec<String>);

struct Reader<F: Fn(&str) -> Option<String>> {
    lookup: F,
    problems: Vec<String>,
}

impl<F: Fn(&str) -> Option<String>> Reader<F> {
    fn optional(&self, name: &str) -> Option<String> {
        (self.lookup)(name)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    }

    fn required(&mut self, name: &str) -> String {
        self.optional(name).unwrap_or_else(|| {
            self.problems.push(format!("{name} is required"));
            String::new()
        })
    }

    fn url(&mut self, name: &str) -> String {
        let value = self.required(name);
        if !value.is_empty() && !(value.starts_with("http://") || value.starts_with("https://")) {
            self.problems.push(format!("{name} must be an http(s) URL"));
        }
        value
    }

    fn list(&mut self, name: &str) -> Vec<String> {
        let value = self.required(name);
        value
            .split(',')
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
            .collect()
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut r = Reader {
            lookup,
            problems: Vec::new(),
        };

        let environment = match r.required("DISCORDY_ENV").as_str() {
            "local" => Environment::Local,
            "production" => Environment::Production,
            "" => Environment::Production,
            other => {
                r.problems.push(format!(
                    "DISCORDY_ENV must be 'local' or 'production', got '{other}'"
                ));
                Environment::Production
            }
        };
        let log_level = r.optional("LOG_LEVEL").unwrap_or_else(|| "info".to_owned());
        let otel_endpoint = r.optional("OTEL_EXPORTER_OTLP_ENDPOINT");
        let internal_service_token = r.required("INTERNAL_SERVICE_TOKEN");
        let kafka_brokers = r.list("KAFKA_BROKERS");
        let database_url = r.required("ADMIN_DATABASE_URL");
        if !database_url.is_empty() && !database_url.starts_with("postgres") {
            r.problems
                .push("ADMIN_DATABASE_URL must be a postgres:// URL".into());
        }
        let api_internal_url = r.url("API_INTERNAL_URL");
        let gateway_internal_url = r.url("GATEWAY_INTERNAL_URL");
        let victoriametrics_url = r.url("VICTORIAMETRICS_URL");
        let etcd_endpoints = r.list("ETCD_ENDPOINTS");

        let allowlist = AdminAllowlist::parse(&r.required("ADMIN_ALLOWED_EMAILS"));
        if allowlist.is_empty() && r.optional("ADMIN_ALLOWED_EMAILS").is_some() {
            r.problems
                .push("ADMIN_ALLOWED_EMAILS must list at least one email".into());
        }

        let team = r.optional("CF_ACCESS_TEAM_DOMAIN");
        let audience = r.optional("CF_ACCESS_AUD");
        let cloudflare_access = match (team, audience) {
            (Some(team_domain), Some(audience)) => Some(CloudflareAccess {
                team_domain,
                audience,
            }),
            (None, None) if environment == Environment::Local => None,
            _ => {
                r.problems.push("CF_ACCESS_TEAM_DOMAIN and CF_ACCESS_AUD are both required (optional only in local)".into());
                None
            }
        };

        if !r.problems.is_empty() {
            return Err(ConfigError(r.problems));
        }
        Ok(Config {
            environment,
            log_level,
            otel_endpoint,
            internal_service_token,
            kafka_brokers,
            database_url,
            api_internal_url,
            gateway_internal_url,
            victoriametrics_url,
            etcd_endpoints,
            allowlist,
            cloudflare_access,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn base() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("DISCORDY_ENV", "production"),
            ("INTERNAL_SERVICE_TOKEN", "t"),
            ("KAFKA_BROKERS", "localhost:19092, other:9092"),
            (
                "ADMIN_DATABASE_URL",
                "postgres://u:p@localhost/discordy_admin",
            ),
            ("API_INTERNAL_URL", "http://localhost:8080"),
            ("GATEWAY_INTERNAL_URL", "http://localhost:4000"),
            ("VICTORIAMETRICS_URL", "http://localhost:8428"),
            ("ETCD_ENDPOINTS", "http://localhost:2379"),
            ("ADMIN_ALLOWED_EMAILS", "ops@discordy.dev"),
            ("CF_ACCESS_TEAM_DOMAIN", "acme"),
            ("CF_ACCESS_AUD", "aud"),
        ])
    }

    fn load(env: &HashMap<&'static str, &'static str>) -> Result<Config, ConfigError> {
        Config::from_lookup(|k| env.get(k).map(|v| v.to_string()))
    }

    #[test]
    fn loads_a_complete_production_config() {
        let config = load(&base()).unwrap();
        assert_eq!(config.environment, Environment::Production);
        assert_eq!(config.kafka_brokers, vec!["localhost:19092", "other:9092"]);
        assert_eq!(config.log_level, "info");
        assert!(!config.accepts_dev_header());
        assert_eq!(config.cloudflare_access.unwrap().team_domain, "acme");
    }

    #[test]
    fn production_requires_cloudflare_access() {
        let mut env = base();
        env.remove("CF_ACCESS_AUD");
        let error = load(&env).unwrap_err();
        assert!(error.0.iter().any(|p| p.contains("CF_ACCESS_AUD")));
    }

    #[test]
    fn local_mode_allows_the_dev_header_without_cloudflare() {
        let mut env = base();
        env.insert("DISCORDY_ENV", "local");
        env.remove("CF_ACCESS_AUD");
        env.remove("CF_ACCESS_TEAM_DOMAIN");
        let config = load(&env).unwrap();
        assert!(config.accepts_dev_header());
        assert_eq!(config.cloudflare_access, None);
    }

    #[test]
    fn reports_every_problem_at_once() {
        let mut env = base();
        env.remove("INTERNAL_SERVICE_TOKEN");
        env.insert("API_INTERNAL_URL", "localhost:8080");
        env.insert("DISCORDY_ENV", "staging");
        env.insert("ADMIN_ALLOWED_EMAILS", " , ");
        let error = load(&env).unwrap_err();
        assert_eq!(error.0.len(), 4, "{error}");
    }
}
