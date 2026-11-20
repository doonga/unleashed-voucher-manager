use std::collections::HashMap;

use backend::environment::Environment;

fn lookup(overrides: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let mut vars: HashMap<String, String> = [
        ("UNLEASHED_URL", "https://unleashed.example.com"),
        ("UNLEASHED_USERNAME", "voucher-manager"),
        ("UNLEASHED_PASSWORD", "secret"),
        ("UNLEASHED_SSID", "Guest"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    for (k, v) in overrides {
        vars.insert(k.to_string(), v.to_string());
    }
    move |name: &str| vars.get(name).cloned()
}

#[test]
fn a_valid_configuration_loads() {
    let env = Environment::from_lookup(lookup(&[])).unwrap();
    assert_eq!(env.unleashed_url, "https://unleashed.example.com");
    assert_eq!(env.rolling_voucher_duration_minutes, 480);
}

#[test]
fn a_zero_rolling_duration_is_refused() {
    let err =
        Environment::from_lookup(lookup(&[("ROLLING_VOUCHER_DURATION_MINUTES", "0")])).unwrap_err();
    assert!(err.contains("ROLLING_VOUCHER_DURATION_MINUTES"), "{err}");
}

#[test]
fn an_unparseable_url_is_refused() {
    for url in ["https://", "https://exa mple.com", "https://[::1"] {
        let err = Environment::from_lookup(lookup(&[("UNLEASHED_URL", url)])).unwrap_err();
        assert!(err.contains("UNLEASHED_URL"), "{url}: {err}");
    }
}

#[test]
fn a_url_without_a_scheme_is_refused() {
    let err = Environment::from_lookup(lookup(&[("UNLEASHED_URL", "unleashed.example.com")]))
        .unwrap_err();
    assert!(err.contains("UNLEASHED_URL"), "{err}");
}
