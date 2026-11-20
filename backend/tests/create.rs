use backend::unleashed::create::{
    CreateOutcome, CreateParams, MAX_BATCH, build_form, duration_for_minutes,
    parse_create_response, sanitize_name, validate_key, validate_name, validate_remarks,
};

fn params(count: u32) -> CreateParams {
    CreateParams {
        count,
        name: "Front Desk".to_string(),
        minutes: 1440,
        share_limit: Some(3),
        key: None,
        remarks: None,
        ssid: "Guest".to_string(),
    }
}

fn field<'a>(form: &'a [(&'static str, String)], name: &str) -> &'a str {
    form.iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("form has no {name}"))
}

#[test]
fn rounds_minutes_up_to_hours() {
    assert_eq!(duration_for_minutes(1), Ok((1, "hour")));
    assert_eq!(duration_for_minutes(61), Ok((2, "hour")));
    assert_eq!(duration_for_minutes(1440), Ok((1, "day")));
    assert_eq!(duration_for_minutes(2880), Ok((2, "day")));
    assert_eq!(duration_for_minutes(10080), Ok((1, "week")));
    assert_eq!(duration_for_minutes(1500), Ok((25, "hour")));
    assert!(duration_for_minutes(0).is_err());
}

#[test]
fn replaces_whitespace_in_names() {
    assert_eq!(sanitize_name("Quick Voucher"), "Quick-Voucher");
    assert_eq!(sanitize_name("a\tb  c"), "a-b--c");
    assert_eq!(validate_name(" Front Desk "), Ok("Front-Desk".to_string()));
}

#[test]
fn rejects_name_characters_the_controller_refuses() {
    for c in [
        '\'', '"', '<', '>', '&', '#', ';', '\\', '`', '|', '!', '$', '(', ')', 'é',
    ] {
        let name = format!("a{c}b");
        let err = validate_name(&name).expect_err(&name);
        assert!(err.contains(c), "{err}");
    }
}

#[test]
fn names_allow_brackets_dots_and_dashes() {
    assert_eq!(
        validate_name("[ROLLING]-20261008125714"),
        Ok("[ROLLING]-20261008125714".to_string())
    );
    assert_eq!(validate_name("lobby.1-a_b"), Ok("lobby.1-a_b".to_string()));
}

#[test]
fn remarks_allow_apostrophes_and_commas() {
    assert_eq!(validate_remarks("Bob's phone, room 4"), Ok(()));
}

#[test]
fn remarks_reject_angle_brackets() {
    let err = validate_remarks("<b>").unwrap_err();
    assert!(err.contains("'<'"), "{err}");
    assert!(validate_remarks("a > b").is_err());
    assert!(validate_remarks("café").is_err());
}

#[test]
fn validates_keys() {
    assert_eq!(validate_key("ab"), Ok("AB".to_string()));
    assert_eq!(validate_key("abc123"), Ok("ABC123".to_string()));
    for bad in [
        "a",
        "a234567890123456b",
        "a b",
        "a#b",
        "a,b",
        "a&b",
        "a+b",
        "a'b",
        "a\"b",
        "a<b",
        "a>b",
    ] {
        assert!(validate_key(bad).is_err(), "{bad}");
    }
}

#[test]
fn single_form() {
    let mut p = params(1);
    p.key = Some("abc123".to_string());
    p.remarks = Some("Bob's phone".to_string());
    let form = build_form(&p).unwrap();
    assert_eq!(field(&form, "gentype"), "single");
    assert_eq!(field(&form, "fullname"), "Front-Desk");
    assert_eq!(field(&form, "duration"), "1");
    assert_eq!(field(&form, "duration-unit"), "day");
    assert_eq!(field(&form, "limitnumber"), "3");
    assert_eq!(field(&form, "key"), "ABC123");
    assert_eq!(field(&form, "remarks"), "Bob's phone");
    assert_eq!(field(&form, "guest-wlan"), "Guest");
    assert_eq!(field(&form, "createToNum"), "");
    assert_eq!(field(&form, "shared"), "true");
}

#[test]
fn no_limit_is_unlimited_and_no_key_lets_the_controller_choose() {
    let mut p = params(1);
    p.share_limit = None;
    let form = build_form(&p).unwrap();
    assert_eq!(field(&form, "limitnumber"), "0");
    assert_eq!(field(&form, "key"), "");
    assert_eq!(field(&form, "remarks"), "");
}

#[test]
fn batch_form() {
    let form = build_form(&params(3)).unwrap();
    assert_eq!(field(&form, "gentype"), "multiple");
    assert_eq!(field(&form, "createToNum"), "3");

    let mut keyed = params(3);
    keyed.key = Some("ABC123".to_string());
    assert!(build_form(&keyed).is_err());

    assert!(build_form(&params(0)).is_err());
    assert!(build_form(&params(MAX_BATCH)).is_ok());
    assert!(build_form(&params(MAX_BATCH + 1)).is_err());
}

#[test]
fn invalid_fields_are_rejected_before_sending() {
    let mut p = params(1);
    p.name = "a&b".to_string();
    assert!(build_form(&p).is_err());

    let mut p = params(1);
    p.remarks = Some("<b>".to_string());
    assert!(build_form(&p).is_err());

    let mut p = params(1);
    p.minutes = 0;
    assert!(build_form(&p).is_err());
}

#[test]
fn parses_single_done() {
    let body = include_str!("fixtures/create_single.txt");
    assert_eq!(parse_create_response(body), CreateOutcome::Accepted);
}

#[test]
fn parses_batch_ok_with_prefix_lines() {
    let body = include_str!("fixtures/create_batch.txt");
    assert_eq!(parse_create_response(body), CreateOutcome::Accepted);
}

#[test]
fn parses_key_duplicated() {
    let body = include_str!("fixtures/create_keydup.txt");
    match parse_create_response(body) {
        CreateOutcome::KeyDuplicated(message) => {
            assert!(message.contains("already exists"), "{message}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn parses_invalid_characters() {
    let body = include_str!("fixtures/create_invalid.txt");
    assert!(matches!(
        parse_create_response(body),
        CreateOutcome::InvalidCharacters(_)
    ));
}

#[test]
fn unreadable_response_fails() {
    assert!(matches!(
        parse_create_response("<html>login</html>"),
        CreateOutcome::Failed(_)
    ));
    assert!(matches!(
        parse_create_response(r#"{"result":"SOMETHING_ELSE","errorMsg":"nope"}"#),
        CreateOutcome::Failed(_)
    ));
}
