use std::time::Duration;

use backend::{
    startup::{describe, retry_delay},
    unleashed::session::SessionError,
};

#[test]
fn retry_delay_doubles_to_five_minutes() {
    let delays: Vec<u64> = (0..=7).map(|n| retry_delay(n).as_secs()).collect();
    assert_eq!(delays, [5, 10, 20, 40, 80, 160, 300, 300]);
    assert_eq!(retry_delay(u32::MAX), Duration::from_secs(300));
}

#[test]
fn auth_errors_name_the_credentials() {
    let text = describe(&SessionError::Auth("refused".into()));
    assert!(text.contains("UNLEASHED_USERNAME"), "{text}");
    assert!(text.contains("UNLEASHED_PASSWORD"), "{text}");
}

#[test]
fn connect_errors_name_the_url() {
    let text = describe(&SessionError::Connect("timed out".into()));
    assert!(text.contains("UNLEASHED_URL"), "{text}");
    assert!(text.contains("UNLEASHED_HAS_VALID_CERT"), "{text}");
    assert!(text.contains("timed out"), "{text}");
}
