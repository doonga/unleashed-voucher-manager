use backend::{environment::Environment, unleashed_api::ApiConfig};

const SECRET: &str = "hunter2-do-not-print";

fn config() -> ApiConfig {
    ApiConfig {
        url: "https://unleashed.example.com".into(),
        username: "voucher-manager".into(),
        password: SECRET.into(),
        ssid: "Guest".into(),
        verify_tls: true,
        timezone: chrono_tz::UTC,
        rolling_voucher_duration_minutes: 480,
    }
}

#[test]
fn api_config_debug_hides_the_password() {
    let text = format!("{:?}", config());
    assert!(!text.contains(SECRET), "{text}");
    assert!(text.contains("<redacted>"), "{text}");
    assert!(text.contains("voucher-manager"), "{text}");
}

#[test]
fn environment_debug_hides_the_password() {
    let env = Environment {
        unleashed_url: "https://unleashed.example.com".into(),
        unleashed_username: "voucher-manager".into(),
        unleashed_password: SECRET.into(),
        unleashed_ssid: "Guest".into(),
        backend_bind_host: "127.0.0.1".into(),
        backend_bind_port: 8080,
        purge_all_expired_vouchers: false,
        rolling_voucher_duration_minutes: 480,
        unleashed_has_valid_cert: true,
        timezone: chrono_tz::UTC,
    };
    let text = format!("{env:?}");
    assert!(!text.contains(SECRET), "{text}");
    assert!(text.contains("<redacted>"), "{text}");
}
