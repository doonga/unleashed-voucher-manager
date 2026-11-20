use backend::tasks::next_midnight;
use chrono::{TimeZone, Timelike};
use chrono_tz::{America, Tz};

fn local(tz: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> chrono::DateTime<Tz> {
    tz.with_ymd_and_hms(y, mo, d, h, mi, 0).earliest().unwrap()
}

#[test]
fn an_ordinary_night() {
    let tz = America::Toronto;
    assert_eq!(
        next_midnight(local(tz, 2026, 11, 20, 15, 0)),
        local(tz, 2026, 11, 21, 0, 0)
    );
}

#[test]
fn just_after_midnight_waits_for_the_next_one() {
    let tz = America::Toronto;
    assert_eq!(
        next_midnight(local(tz, 2026, 11, 21, 0, 1)),
        local(tz, 2026, 11, 22, 0, 0)
    );
}

#[test]
fn a_skipped_midnight_runs_at_the_first_valid_time() {
    // Chile moves its clocks from 00:00 to 01:00 on 2026-09-06.
    let tz = America::Santiago;
    let next = next_midnight(local(tz, 2026, 9, 5, 23, 30));
    assert_eq!(next, local(tz, 2026, 9, 6, 1, 0));
}

#[test]
fn a_repeated_midnight_runs_at_the_first_one() {
    // Cuba moves its clocks from 01:00 back to 00:00 on 2026-11-01.
    let tz = America::Havana;
    let next = next_midnight(local(tz, 2026, 10, 31, 23, 30));
    assert_eq!(next.hour(), 0);
    assert_eq!(
        next,
        tz.with_ymd_and_hms(2026, 11, 1, 0, 0, 0)
            .earliest()
            .unwrap()
    );
    assert!(next < tz.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).latest().unwrap());
}
