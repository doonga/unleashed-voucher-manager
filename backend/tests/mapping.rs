use backend::unleashed::{
    guest::{GuestPass, parse_guest_list},
    mapping::{
        ROLLING_PREFIX, current_rolling, format_time, is_expired, is_rolling, is_used,
        rolling_name, to_voucher,
    },
};
use chrono::TimeZone;
use chrono_tz::{America, Tz};

const TZ: Tz = America::Toronto;
const NOW: i64 = 1_795_203_000;

fn fixture() -> Vec<GuestPass> {
    parse_guest_list(include_str!("fixtures/guest_list.xml")).unwrap()
}

fn pass(id: &str, name: &str) -> GuestPass {
    GuestPass {
        id: id.to_string(),
        name: name.to_string(),
        key: "123456".to_string(),
        remarks: String::new(),
        share_number: 1,
        valid_time: 3600,
        create_time: NOW - 60,
        start_time: Some(NOW - 60),
        expire_time: Some(NOW + 3540),
        used: false,
        client_macs: vec![],
    }
}

#[test]
fn maps_every_field() {
    let passes = fixture();
    let front = passes.iter().find(|p| p.id == "1").unwrap();
    let v = to_voucher(front, TZ, NOW);
    assert_eq!(v.id, "1");
    assert_eq!(v.created_at, "2026-11-20 14:29:12");
    assert_eq!(v.name, "front-desk");
    assert_eq!(v.code, "FRONT1");
    assert_eq!(v.authorized_guest_limit, Some(3));
    assert_eq!(v.authorized_guest_count, 0);
    assert_eq!(v.activated_at, None);
    assert_eq!(v.expires_at.as_deref(), Some("2026-11-22 14:29:12"));
    assert!(!v.expired);
    assert_eq!(v.time_limit_minutes, 2880);
    assert_eq!(v.remarks, "Bob's phone, room 4");
}

#[test]
fn counts_clients_and_marks_activation() {
    let passes = fixture();
    let lobby = passes.iter().find(|p| p.id == "5").unwrap();
    let v = to_voucher(lobby, TZ, NOW);
    assert_eq!(v.authorized_guest_count, 1);
    assert!(v.activated_at.is_some());
    assert!(is_used(lobby));
}

#[test]
fn zero_share_limit_is_unlimited() {
    let mut p = pass("1", "a");
    p.share_number = 0;
    assert_eq!(to_voucher(&p, TZ, NOW).authorized_guest_limit, None);
}

#[test]
fn activated_only_when_used() {
    let p = pass("1", "a");
    assert!(!is_used(&p));
    assert_eq!(to_voucher(&p, TZ, NOW).activated_at, None);

    let used = GuestPass {
        used: true,
        ..pass("1", "a")
    };
    assert!(is_used(&used));
    assert!(to_voucher(&used, TZ, NOW).activated_at.is_some());

    let joined = GuestPass {
        client_macs: vec!["00:00:00:00:00:09".into()],
        ..pass("1", "a")
    };
    assert!(is_used(&joined));
}

#[test]
fn expiry_is_inclusive() {
    let p = GuestPass {
        expire_time: Some(NOW),
        ..pass("1", "a")
    };
    assert!(is_expired(&p, NOW));
    assert!(to_voucher(&p, TZ, NOW).expired);
    let p = GuestPass {
        expire_time: Some(NOW + 1),
        ..pass("1", "a")
    };
    assert!(!is_expired(&p, NOW));
}

#[test]
fn a_pass_without_expiry_never_expires() {
    let passes = fixture();
    let p = passes.iter().find(|p| p.id == "6").unwrap();
    assert!(!is_expired(p, NOW));
    let v = to_voucher(p, TZ, NOW);
    assert!(!v.expired);
    assert_eq!(v.expires_at, None);
}

#[test]
fn rolling_name_format() {
    let created = TZ.with_ymd_and_hms(2026, 10, 8, 12, 57, 14).unwrap();
    assert_eq!(rolling_name(created), "[ROLLING]-20261008125714");
    assert!(rolling_name(created).starts_with(ROLLING_PREFIX));
    assert!(is_rolling(&pass("1", "[ROLLING]-20261008125714")));
    assert!(!is_rolling(&pass("1", "[ROLLING] 20261008125714")));
}

#[test]
fn current_rolling_is_the_newest_unused_unexpired() {
    let passes = vec![
        GuestPass {
            create_time: NOW - 30,
            ..pass("1", "[ROLLING]-older")
        },
        GuestPass {
            create_time: NOW - 10,
            used: true,
            ..pass("2", "[ROLLING]-newer-used")
        },
        GuestPass {
            create_time: NOW - 5,
            expire_time: Some(NOW),
            ..pass("3", "[ROLLING]-expired")
        },
        GuestPass {
            create_time: NOW - 1,
            ..pass("4", "not rolling")
        },
        GuestPass {
            create_time: NOW - 20,
            ..pass("5", "[ROLLING]-middle")
        },
    ];
    assert_eq!(
        current_rolling(&passes, NOW).map(|p| p.id.as_str()),
        Some("5")
    );
}

#[test]
fn no_current_rolling_when_all_used_or_expired() {
    let passes = vec![
        GuestPass {
            used: true,
            ..pass("1", "[ROLLING]-a")
        },
        GuestPass {
            client_macs: vec!["00:00:00:00:00:09".into()],
            ..pass("2", "[ROLLING]-b")
        },
        GuestPass {
            expire_time: Some(NOW - 1),
            ..pass("3", "[ROLLING]-c")
        },
    ];
    assert!(current_rolling(&passes, NOW).is_none());
}

#[test]
fn formats_in_the_configured_timezone() {
    assert_eq!(format_time(1_795_202_952, TZ), "2026-11-20 14:29:12");
    assert_eq!(
        format_time(1_795_202_952, chrono_tz::UTC),
        "2026-11-20 19:29:12"
    );
}
