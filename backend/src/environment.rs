use std::{env, sync::OnceLock};

use chrono_tz::Tz;
use tracing::{error, info};

const DEFAULT_BACKEND_BIND_HOST: &str = "127.0.0.1";
const DEFAULT_BACKEND_BIND_PORT: u16 = 8080;
const DEFAULT_ROLLING_VOUCHER_DURATION_MINUTES: u64 = 480;

pub static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();

#[derive(Clone)]
pub struct Environment {
    pub unleashed_url: String,
    pub unleashed_username: String,
    pub unleashed_password: String,
    pub unleashed_ssid: String,
    pub backend_bind_host: String,
    pub backend_bind_port: u16,
    pub purge_all_expired_vouchers: bool,
    pub rolling_voucher_duration_minutes: u64,
    pub unleashed_has_valid_cert: bool,
    pub timezone: Tz,
}

impl Environment {
    pub fn try_new() -> Result<Self, String> {
        #[cfg(feature = "dotenv")]
        dotenvy::dotenv().map_err(|e| format!("Failed to load .env file: {e}"))?;

        Self::from_lookup(|name| env::var(name).ok())
    }

    /// Read and check the configuration from `get`, which returns a
    /// variable's value if it is set. Invalid settings are refused here, at
    /// startup, rather than failing every request later.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let required = |name: &str| match get(name) {
            Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
            Some(_) => Err(format!("{name} is empty")),
            None => Err(format!("{name} is not set")),
        };

        let unleashed_url = required("UNLEASHED_URL")?;
        if !unleashed_url.starts_with("http://") && !unleashed_url.starts_with("https://") {
            return Err("UNLEASHED_URL must start with http:// or https://".to_string());
        }
        match reqwest::Url::parse(&unleashed_url) {
            Ok(url) if url.host_str().is_some_and(|h| !h.is_empty()) => {}
            Ok(_) => return Err("UNLEASHED_URL has no host".to_string()),
            Err(e) => return Err(format!("UNLEASHED_URL is not a valid URL: {e}")),
        }
        let unleashed_username = required("UNLEASHED_USERNAME")?;
        let unleashed_password = get("UNLEASHED_PASSWORD")
            .filter(|v| !v.is_empty())
            .ok_or("UNLEASHED_PASSWORD is not set")?;
        let unleashed_ssid = required("UNLEASHED_SSID")?;

        let backend_bind_host: String =
            get("BACKEND_BIND_HOST").unwrap_or(DEFAULT_BACKEND_BIND_HOST.to_owned());
        let backend_bind_port: u16 = match get("BACKEND_BIND_PORT") {
            Some(port_str) => port_str
                .parse()
                .map_err(|e| format!("Invalid BACKEND_BIND_PORT: {e}"))?,
            None => DEFAULT_BACKEND_BIND_PORT,
        };

        let rolling_voucher_duration_minutes = match get("ROLLING_VOUCHER_DURATION_MINUTES") {
            Some(val) => val
                .parse()
                .map_err(|e| format!("Invalid ROLLING_VOUCHER_DURATION_MINUTES: {e}"))?,
            None => DEFAULT_ROLLING_VOUCHER_DURATION_MINUTES,
        };
        if rolling_voucher_duration_minutes == 0 {
            return Err("ROLLING_VOUCHER_DURATION_MINUTES must be at least 1".to_string());
        }

        let purge_all_expired_vouchers: bool = match get("PURGE_ALL_EXPIRED_VOUCHERS") {
            Some(val) => Self::parse_bool(&val)
                .map_err(|e| format!("Invalid PURGE_ALL_EXPIRED_VOUCHERS: {e}"))?,
            None => false,
        };

        let unleashed_has_valid_cert: bool = match get("UNLEASHED_HAS_VALID_CERT") {
            Some(val) => Self::parse_bool(&val)
                .map_err(|e| format!("Invalid UNLEASHED_HAS_VALID_CERT: {e}"))?,
            None => true,
        };

        let timezone: Tz = match get("TIMEZONE") {
            Some(s) => match s.parse() {
                Ok(tz) => {
                    info!("Using timezone: {}", s);
                    tz
                }
                Err(_) => {
                    error!("Using UTC, could not parse timezone: {}", s);
                    Tz::UTC
                }
            },
            None => {
                info!("TIMEZONE environment variable not set, defaulting to UTC");
                Tz::UTC
            }
        };

        Ok(Self {
            unleashed_url,
            unleashed_username,
            unleashed_password,
            unleashed_ssid,
            backend_bind_host,
            backend_bind_port,
            rolling_voucher_duration_minutes,
            purge_all_expired_vouchers,
            unleashed_has_valid_cert,
            timezone,
        })
    }

    fn parse_bool(s: &str) -> Result<bool, String> {
        match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            _ => Err(format!("Boolean value must be true or false, found: {s}")),
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field("unleashed_url", &self.unleashed_url)
            .field("unleashed_username", &self.unleashed_username)
            .field("unleashed_password", &"<redacted>")
            .field("unleashed_ssid", &self.unleashed_ssid)
            .field("backend_bind_host", &self.backend_bind_host)
            .field("backend_bind_port", &self.backend_bind_port)
            .field(
                "purge_all_expired_vouchers",
                &self.purge_all_expired_vouchers,
            )
            .field(
                "rolling_voucher_duration_minutes",
                &self.rolling_voucher_duration_minutes,
            )
            .field("unleashed_has_valid_cert", &self.unleashed_has_valid_cert)
            .field("timezone", &self.timezone)
            .finish()
    }
}
