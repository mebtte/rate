use std::{ffi::OsString, net::IpAddr};

use clap::Parser;
use tracing_subscriber::filter::LevelFilter;

#[derive(Parser)]
#[command(version, about = "Serve the rate network speed test")]
pub struct Config {
    /// Address to listen on.
    #[arg(long, env = "RATE_HOST", default_value = "127.0.0.1")]
    pub host: IpAddr,

    /// Port to listen on. Use 0 to select a free port.
    #[arg(long, env = "RATE_PORT", default_value_t = 8080)]
    pub port: u16,

    /// URL path prefix for the page, assets and API.
    #[arg(long, env = "RATE_BASE_PATH", default_value = "/", value_parser = base_path)]
    pub base_path: String,

    /// Reverse proxy address allowed to supply X-Forwarded-For. Repeat for multiple hops.
    #[arg(long, env = "RATE_TRUSTED_PROXY", value_delimiter = ',')]
    pub trusted_proxy: Vec<IpAddr>,

    /// Log level (off, error, warn, info, debug or trace).
    #[arg(long, env = "RATE_LOG_LEVEL", default_value = "info")]
    pub log_level: LevelFilter,

    /// Token for online IPinfo Lite lookup.
    #[arg(long, env = "RATE_IPINFO_API_KEY", hide_env_values = true)]
    pub ipinfo_api_key: Option<String>,

    /// HTTP Basic authentication credentials.
    #[arg(
        long,
        env = "RATE_BASIC_AUTH",
        hide_env_values = true,
        allow_hyphen_values = true,
        value_name = "USER:PASSWORD"
    )]
    basic_auth: Option<OsString>,
}

impl Config {
    pub fn basic_auth(&self) -> Result<Option<(&str, &str)>, &'static str> {
        // Validate after parsing so Clap never includes an invalid secret in its error output.
        self.basic_auth.as_deref().map(|credentials| {
            credentials.to_str()
                .and_then(|credentials| credentials.split_once(':'))
                .filter(|(username, password)| valid_credential(username) && valid_credential(password))
                .ok_or("Basic auth requires USER:PASSWORD with nonempty UTF-8 values and no control characters")
        }).transpose()
    }

    pub fn validate_ipinfo_key(&self) -> Result<(), &'static str> {
        if self
            .ipinfo_api_key
            .as_deref()
            .is_some_and(|key| key.is_empty() || !key.bytes().all(|byte| byte.is_ascii_graphic()))
        {
            return Err("IPinfo API key must consist of visible ASCII characters");
        }
        Ok(())
    }
}

fn base_path(value: &str) -> Result<String, &'static str> {
    if value == "/" {
        return Ok(value.into());
    }

    let path = value.strip_suffix('/').unwrap_or(value);
    let segments = path
        .strip_prefix('/')
        .ok_or("base path must start with /")?;
    if segments.split('/').any(|segment| {
        segment.is_empty()
            || matches!(segment, "." | "..")
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte))
    }) {
        return Err("base path must contain valid URL path segments separated by /");
    }
    Ok(path.into())
}

fn valid_credential(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates_base_path() {
        assert_eq!(base_path("/"), Ok("/".into()));
        assert_eq!(
            base_path("/tools/speedtest/"),
            Ok("/tools/speedtest".into())
        );
        for invalid in [
            "",
            "speedtest",
            "//",
            "/tools//test",
            "/../test",
            "/%2f",
            "/{name}",
            "/x?q=1",
            "/x#y",
            "/x\\y",
        ] {
            assert!(base_path(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn basic_auth_splits_on_the_first_colon() {
        let config = Config::try_parse_from(["rate", "--basic-auth", "-alice:pass:word"]).unwrap();
        assert_eq!(config.basic_auth(), Ok(Some(("-alice", "pass:word"))));
    }

    #[test]
    fn basic_auth_rejects_incomplete_values_and_control_characters() {
        for invalid in [
            "",
            "username",
            ":password",
            "username:",
            "bad\nuser:password",
            "username:bad\tpassword",
        ] {
            let config = Config::try_parse_from(["rate", "--basic-auth", invalid]).unwrap();
            assert!(config.basic_auth().is_err());
        }
    }
}
