use chrono::{DateTime, Duration, LocalResult, TimeZone, Utc};
use chrono_tz::Tz;
use tokio::time::sleep;
use tracing::{error, info};

use crate::handlers::{delete_expired_handler, delete_expired_rolling_handler};

pub async fn run_daily_purge(timezone: Tz, purge_all: bool) {
    let voucher_desc = if purge_all {
        "expired vouchers"
    } else {
        "expired rolling vouchers"
    };

    loop {
        let now = Utc::now().with_timezone(&timezone);
        let next_midnight = next_midnight(now);

        let delta = next_midnight - now;
        let duration = delta.to_std().unwrap_or_default();

        info!(
            "Next purge of {} at midnight ({}), in {} hours and {} minutes...",
            voucher_desc,
            timezone,
            delta.num_hours(),
            delta.num_minutes() % 60
        );

        sleep(duration).await;

        info!("Purging {}...", voucher_desc);

        let result = if purge_all {
            delete_expired_handler().await
        } else {
            delete_expired_rolling_handler().await
        };
        match result {
            Ok(response) => info!("Deleted {} {}", response.vouchers_deleted, voucher_desc),
            Err(code) => error!("Failed to delete {}: {}", voucher_desc, code),
        };
    }
}

/// The next local midnight after `now`. Where daylight saving skips
/// midnight, the first valid local time after it; where midnight happens
/// twice, the first one.
pub fn next_midnight(now: DateTime<Tz>) -> DateTime<Tz> {
    let timezone = now.timezone();
    let mut day = now.date_naive();
    loop {
        day = day.succ_opt().expect("next day is representable");
        let midnight = day.and_hms_opt(0, 0, 0).expect("midnight exists");
        let found = match timezone.from_local_datetime(&midnight) {
            LocalResult::Single(t) => Some(t),
            LocalResult::Ambiguous(first, _) => Some(first),
            // Midnight was skipped: step forward a minute at a time to the
            // first local time that exists.
            LocalResult::None => (1..=24 * 60).find_map(|m| {
                timezone
                    .from_local_datetime(&(midnight + Duration::minutes(m)))
                    .earliest()
            }),
        };
        if let Some(t) = found.filter(|t| *t > now) {
            return t;
        }
    }
}
