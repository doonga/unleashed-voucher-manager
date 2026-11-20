//! Turning guest passes into the vouchers the frontend shows, and the
//! rolling-voucher rules.

use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;

use crate::{models::Voucher, unleashed::guest::GuestPass};

/// Rolling vouchers are recognised by this name prefix. The controller
/// rejects whitespace in names, so it ends in a dash.
pub const ROLLING_PREFIX: &str = "[ROLLING]-";

const DATE_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const ROLLING_TIME_FORMAT: &str = "%Y%m%d%H%M%S";

/// Unix seconds as local time in `timezone`.
pub fn format_time(unix: i64, timezone: Tz) -> String {
    match timezone.timestamp_opt(unix, 0).single() {
        Some(time) => time.format(DATE_TIME_FORMAT).to_string(),
        None => unix.to_string(),
    }
}

/// A pass is used once a device has joined it. `start-time` is not a sign of
/// use: when validity counts from issue, the controller sets it at creation.
pub fn is_used(pass: &GuestPass) -> bool {
    pass.used || !pass.client_macs.is_empty()
}

/// A pass without an expiry time never counts as expired.
pub fn is_expired(pass: &GuestPass, now: i64) -> bool {
    pass.expire_time.is_some_and(|expire| expire <= now)
}

pub fn to_voucher(pass: &GuestPass, timezone: Tz, now: i64) -> Voucher {
    Voucher {
        id: pass.id.clone(),
        created_at: format_time(pass.create_time, timezone),
        name: pass.name.clone(),
        code: pass.key.clone(),
        authorized_guest_limit: Some(pass.share_number).filter(|n| *n > 0),
        authorized_guest_count: pass.client_macs.len() as u64,
        activated_at: if is_used(pass) {
            Some(format_time(
                pass.start_time.unwrap_or(pass.create_time),
                timezone,
            ))
        } else {
            None
        },
        expires_at: pass.expire_time.map(|t| format_time(t, timezone)),
        expired: is_expired(pass, now),
        time_limit_minutes: pass.valid_time / 60,
        remarks: pass.remarks.clone(),
    }
}

pub fn is_rolling(pass: &GuestPass) -> bool {
    pass.name.starts_with(ROLLING_PREFIX)
}

pub fn rolling_name(created: DateTime<Tz>) -> String {
    format!("{ROLLING_PREFIX}{}", created.format(ROLLING_TIME_FORMAT))
}

/// The rolling voucher waiting to be handed out: the newest one that is
/// neither used nor expired.
pub fn current_rolling(passes: &[GuestPass], now: i64) -> Option<&GuestPass> {
    passes
        .iter()
        .filter(|p| is_rolling(p) && !is_used(p) && !is_expired(p, now))
        .max_by_key(|p| p.create_time)
}
