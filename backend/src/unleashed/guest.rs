//! Parsing of the controller's guest pass list (`getconf comp='guest-list'`).

/// One guest pass as the controller lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestPass {
    pub id: String,
    /// `full-name`, falling back to `name`.
    pub name: String,
    /// `x-key`, falling back to `key`.
    pub key: String,
    pub remarks: String,
    /// Devices allowed to share the pass; `0` is unlimited.
    pub share_number: u64,
    /// Validity in seconds.
    pub valid_time: u64,
    /// Unix seconds.
    pub create_time: i64,
    /// Unix seconds; `None` when empty or zero.
    pub start_time: Option<i64>,
    /// Unix seconds; `None` when empty or zero.
    pub expire_time: Option<i64>,
    pub used: bool,
    pub client_macs: Vec<String>,
}

/// Parse a guest-list response into passes, in the order the controller sent them.
pub fn parse_guest_list(xml: &str) -> Result<Vec<GuestPass>, String> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(xml, options)
        .map_err(|e| format!("unreadable guest list: {e}"))?;

    Ok(doc
        .descendants()
        .filter(|node| node.has_tag_name("guest"))
        .map(|guest| {
            let text = |name: &str| guest.attribute(name).unwrap_or_default().to_string();
            let first = |a: &str, b: &str| {
                guest
                    .attribute(a)
                    .filter(|v| !v.is_empty())
                    .or_else(|| guest.attribute(b))
                    .unwrap_or_default()
                    .to_string()
            };
            let number = |name: &str| -> i64 {
                guest
                    .attribute(name)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0)
            };
            let time = |name: &str| Some(number(name)).filter(|t| *t > 0);

            GuestPass {
                id: text("id"),
                name: first("full-name", "name"),
                key: first("x-key", "key"),
                remarks: text("remarks"),
                share_number: number("share-number").max(0) as u64,
                valid_time: number("valid-time").max(0) as u64,
                create_time: number("create-time"),
                start_time: time("start-time"),
                expire_time: time("expire-time"),
                used: guest.attribute("used") == Some("true"),
                client_macs: guest
                    .children()
                    .filter(|c| c.has_tag_name("client"))
                    .filter_map(|c| c.attribute("mac").map(str::to_string))
                    .collect(),
            }
        })
        .collect())
}
