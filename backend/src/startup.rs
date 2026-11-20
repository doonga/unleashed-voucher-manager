//! Connecting to the controller at startup.

use std::time::Duration;

use crate::unleashed::session::SessionError;

const FIRST_DELAY_SECS: u64 = 5;
const MAX_DELAY_SECS: u64 = 300;

/// Delay before connection attempt `attempt + 1`: 5 seconds, doubling, at
/// most 5 minutes. Retrying slowly rather than exiting lets a corrected
/// password or a returning controller recover without a restart, without
/// hammering the controller with failed logins.
pub fn retry_delay(attempt: u32) -> Duration {
    let factor = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
    Duration::from_secs(FIRST_DELAY_SECS.saturating_mul(factor).min(MAX_DELAY_SECS))
}

/// What to tell the operator about a failed connection, naming the setting
/// to check but never its value.
pub fn describe(error: &SessionError) -> String {
    match error {
        SessionError::Auth(e) => format!(
            "The controller refused the login ({e}). Check UNLEASHED_USERNAME and \
             UNLEASHED_PASSWORD; repeated failed logins can make the controller refuse \
             this address for a while."
        ),
        SessionError::Connect(e) => {
            format!("The controller could not be reached ({e}). Check UNLEASHED_URL.")
        }
    }
}
