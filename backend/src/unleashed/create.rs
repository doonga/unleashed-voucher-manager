//! Building `mon_createguest.jsp` requests and reading their responses.

/// The most passes one batch create may ask for.
pub const MAX_BATCH: u32 = 100;

const MINUTES_PER_HOUR: u64 = 60;
const HOURS_PER_DAY: u64 = 24;
const HOURS_PER_WEEK: u64 = 7 * HOURS_PER_DAY;
const MIN_KEY_LENGTH: usize = 2;
const MAX_KEY_LENGTH: usize = 16;

/// Characters the controller refuses in a pass name: it answers `OK` and
/// creates nothing.
const NAME_FORBIDDEN: &[char] = &[
    '\'', '"', '<', '>', '&', '#', ';', '\\', '`', '|', '!', '$', '(', ')',
];
/// Characters the controller refuses in remarks ("Invalid Characters").
const REMARKS_FORBIDDEN: &[char] = &['<', '>'];
/// Characters the controller refuses in a custom key.
const KEY_FORBIDDEN: &[char] = &['#', '&', '+', '"', '\'', '<', '>', ','];

/// What to create.
#[derive(Debug, Clone)]
pub struct CreateParams {
    pub count: u32,
    pub name: String,
    pub minutes: u64,
    /// Devices per pass; `None` is unlimited.
    pub share_limit: Option<u64>,
    pub key: Option<String>,
    pub remarks: Option<String>,
    /// The guest WLAN the passes are for.
    pub ssid: String,
}

/// How the controller answered a create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    Accepted,
    KeyDuplicated(String),
    InvalidCharacters(String),
    Failed(String),
}

/// Round minutes up to whole hours and express them in the largest unit
/// that divides them exactly. The controller cannot take minutes.
pub fn duration_for_minutes(minutes: u64) -> Result<(u64, &'static str), String> {
    if minutes == 0 {
        return Err("duration must be at least one minute".to_string());
    }
    let hours = minutes.div_ceil(MINUTES_PER_HOUR);
    Ok(if hours.is_multiple_of(HOURS_PER_WEEK) {
        (hours / HOURS_PER_WEEK, "week")
    } else if hours.is_multiple_of(HOURS_PER_DAY) {
        (hours / HOURS_PER_DAY, "day")
    } else {
        (hours, "hour")
    })
}

/// Replace each whitespace character with `-`; the controller silently
/// creates nothing for names containing whitespace.
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .collect()
}

fn check_ascii(field: &str, value: &str) -> Result<(), String> {
    match value.chars().find(|c| !(' '..='~').contains(c)) {
        Some(c) => Err(format!("{field} cannot contain '{c}'")),
        None => Ok(()),
    }
}

fn check_forbidden(field: &str, value: &str, forbidden: &[char]) -> Result<(), String> {
    match value.chars().find(|c| forbidden.contains(c)) {
        Some(c) => Err(format!("{field} cannot contain '{c}'")),
        None => Ok(()),
    }
}

/// Trim and sanitise a name, then reject characters the controller refuses.
pub fn validate_name(name: &str) -> Result<String, String> {
    let name = sanitize_name(name.trim());
    check_ascii("name", &name)?;
    check_forbidden("name", &name, NAME_FORBIDDEN)?;
    Ok(name)
}

/// Reject remarks the controller refuses.
pub fn validate_remarks(remarks: &str) -> Result<(), String> {
    check_ascii("remarks", remarks)?;
    check_forbidden("remarks", remarks, REMARKS_FORBIDDEN)
}

/// Check a custom key and uppercase it, as the controller stores it.
pub fn validate_key(key: &str) -> Result<String, String> {
    let length = key.chars().count();
    if !(MIN_KEY_LENGTH..=MAX_KEY_LENGTH).contains(&length) {
        return Err(format!(
            "key must be {MIN_KEY_LENGTH} to {MAX_KEY_LENGTH} characters"
        ));
    }
    if key.chars().any(char::is_whitespace) {
        return Err("key cannot contain whitespace".to_string());
    }
    check_ascii("key", key)?;
    check_forbidden("key", key, KEY_FORBIDDEN)?;
    Ok(key.to_uppercase())
}

/// The form fields for `mon_createguest.jsp`.
pub fn build_form(params: &CreateParams) -> Result<Vec<(&'static str, String)>, String> {
    if params.count == 0 || params.count > MAX_BATCH {
        return Err(format!("count must be 1 to {MAX_BATCH}"));
    }
    let single = params.count == 1;
    let key = match params
        .key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        Some(_) if !single => return Err("a custom key needs a count of 1".to_string()),
        Some(key) => validate_key(key)?,
        None => String::new(),
    };
    let name = validate_name(&params.name)?;
    let remarks = params.remarks.clone().unwrap_or_default();
    validate_remarks(&remarks)?;
    let (duration, unit) = duration_for_minutes(params.minutes)?;

    Ok(vec![
        (
            "gentype",
            if single { "single" } else { "multiple" }.to_string(),
        ),
        ("fullname", name),
        ("remarks", remarks),
        ("duration", duration.to_string()),
        ("duration-unit", unit.to_string()),
        ("key", key),
        (
            "createToNum",
            if single {
                String::new()
            } else {
                params.count.to_string()
            },
        ),
        ("batchpass", String::new()),
        ("guest-wlan", params.ssid.clone()),
        ("shared", "true".to_string()),
        ("reauth", "false".to_string()),
        ("reauth-time", String::new()),
        ("reauth-unit", "min".to_string()),
        ("email", String::new()),
        ("countrycode", String::new()),
        ("phonenumber", String::new()),
        ("limitnumber", params.share_limit.unwrap_or(0).to_string()),
    ])
}

/// Read the controller's answer. Batch answers start with script lines before
/// the JSON object, so everything before the first `{` is skipped.
pub fn parse_create_response(body: &str) -> CreateOutcome {
    let Some(start) = body.find('{') else {
        return CreateOutcome::Failed("unreadable response".to_string());
    };
    let value: serde_json::Value = match serde_json::from_str(&body[start..]) {
        Ok(value) => value,
        Err(e) => return CreateOutcome::Failed(format!("unreadable response: {e}")),
    };
    let result = value["result"].as_str().unwrap_or_default();
    let message = value["errorMsg"]
        .as_str()
        .filter(|m| *m != "~~" && *m != "null")
        .unwrap_or_default()
        .to_string();

    match result {
        "DONE" | "OK" => CreateOutcome::Accepted,
        "KEY_DUPLICATED" => CreateOutcome::KeyDuplicated(message),
        _ if message.contains("Invalid Characters") => CreateOutcome::InvalidCharacters(message),
        _ => CreateOutcome::Failed(if message.is_empty() {
            format!("controller answered {result:?}")
        } else {
            message
        }),
    }
}
