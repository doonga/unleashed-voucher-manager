use backend::unleashed::guest::{GuestPass, parse_guest_list};

const FIXTURE: &str = include_str!("fixtures/guest_list.xml");

fn by_id<'a>(passes: &'a [GuestPass], id: &str) -> &'a GuestPass {
    passes.iter().find(|p| p.id == id).expect("pass in fixture")
}

#[test]
fn parses_every_pass() {
    let passes = parse_guest_list(FIXTURE).unwrap();
    assert_eq!(passes.len(), 6);

    let front = by_id(&passes, "1");
    assert_eq!(front.name, "front-desk");
    assert_eq!(front.key, "FRONT1");
    assert_eq!(front.remarks, "Bob's phone, room 4");
    assert_eq!(front.share_number, 3);
    assert_eq!(front.valid_time, 172_800);
    assert_eq!(front.create_time, 1_795_202_952);
    assert_eq!(front.start_time, Some(1_795_202_952));
    assert_eq!(front.expire_time, Some(1_795_375_752));
}

#[test]
fn prefers_full_name_and_x_key() {
    let xml = r#"<ajax-response><response><guest id="9" name="short" full-name="long name" key="AAA" x-key="BBB" create-time="1" valid-time="3600" /></response></ajax-response>"#;
    let passes = parse_guest_list(xml).unwrap();
    assert_eq!(passes[0].name, "long name");
    assert_eq!(passes[0].key, "BBB");

    let xml = r#"<ajax-response><response><guest id="9" name="short" key="AAA" create-time="1" valid-time="3600" /></response></ajax-response>"#;
    let passes = parse_guest_list(xml).unwrap();
    assert_eq!(passes[0].name, "short");
    assert_eq!(passes[0].key, "AAA");
}

#[test]
fn reads_client_macs_and_used() {
    let passes = parse_guest_list(FIXTURE).unwrap();
    let lobby = by_id(&passes, "5");
    assert!(lobby.used);
    assert_eq!(lobby.client_macs, vec!["00:00:00:00:00:01".to_string()]);

    let front = by_id(&passes, "1");
    assert!(!front.used);
    assert!(front.client_macs.is_empty());
}

#[test]
fn empty_times_are_none() {
    let passes = parse_guest_list(FIXTURE).unwrap();
    let no_expiry = by_id(&passes, "6");
    assert_eq!(no_expiry.start_time, None);
    assert_eq!(no_expiry.expire_time, None);

    let xml = r#"<r><guest id="7" full-name="z" start-time="0" expire-time="0" create-time="1" valid-time="1" /></r>"#;
    let passes = parse_guest_list(xml).unwrap();
    assert_eq!(passes[0].start_time, None);
    assert_eq!(passes[0].expire_time, None);
}

#[test]
fn missing_numbers_default_to_zero() {
    let xml = r#"<r><guest id="7" full-name="z" /></r>"#;
    let passes = parse_guest_list(xml).unwrap();
    assert_eq!(passes[0].share_number, 0);
    assert_eq!(passes[0].valid_time, 0);
    assert_eq!(passes[0].create_time, 0);
}

#[test]
fn empty_list_is_empty() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?><!DOCTYPE ajax-response><ajax-response><response type="object" id="guest-list.1" /></ajax-response>"#;
    assert!(parse_guest_list(xml).unwrap().is_empty());
}

#[test]
fn garbage_is_an_error() {
    assert!(parse_guest_list("not xml").is_err());
}
