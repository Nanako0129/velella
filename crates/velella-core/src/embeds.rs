//! Message embeds (wave 3 E, SECURITY slice: untrusted content). Parsed once, where a message's
//! body is (`messages.rs` `parse` and `Stored::merge`); `row()` only copies the result.
//!
//! - At most `MAX_EMBEDS` embeds and `MAX_FIELDS` fields each. Text is cut by `char` to Discord's
//!   per-field caps, then to the 6000-character budget Discord applies over all of a message's
//!   embeds, in display order (author name, title, description, field names and values, footer
//!   text); text past the budget is dropped.
//! - Descriptions and field values are markdown, parsed with core's limits and never highlighted.
//!   A cut one containing `||` becomes one spoiler (the markdown fallback rule), so a cut can never
//!   open a spoiler.
//! - Images and icons come only from `proxy_url` / `proxy_icon_url` through `media_url`; the
//!   original `url`/`icon_url` of images, icons and videos is never read. Links (`url`,
//!   `author.url`, `provider.url`) pass the message links' check plus a 2048-byte cap.
use crate::{
    markdown::{self, Block, Inline},
    media::media_url,
};
use serde_json::Value;

const MAX_EMBEDS: usize = 10;
const MAX_FIELDS: usize = 25;
const TITLE_CHARS: usize = 256;
const AUTHOR_CHARS: usize = 256;
const FIELD_NAME_CHARS: usize = 256;
const DESCRIPTION_CHARS: usize = 4096;
const FIELD_VALUE_CHARS: usize = 1024;
const FOOTER_CHARS: usize = 2048;
/// Discord's combined limit over title, description, field names and values, footer text and
/// author name of all embeds of one message.
const TOTAL_CHARS: usize = 6000;
const LINK_BYTES: usize = 2048;
/// Largest width or height an embed image may claim.
const MAX_IMAGE_SIDE: u64 = 16384;

/// A checked link target: the canonical http(s) URL and its ASCII host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedLink {
    pub url: String,
    pub host: String,
}

/// A thumbnail or image: a `media_url` proxy URL with both sides in 1...16384.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedImage {
    pub url: String,
    pub width: u32,
    pub height: u32,
    /// Alt text, shown verbatim.
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedField {
    pub name: String,
    pub value: Vec<Block>,
    pub inline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Embed {
    pub title: Option<String>,
    /// The embed's canonical link (the title's target).
    pub url: Option<EmbedLink>,
    pub description: Vec<Block>,
    /// 0xRRGGBB; `None` draws the default bar.
    pub color: Option<u32>,
    pub timestamp_ms: Option<i64>,
    pub author_name: Option<String>,
    pub author_url: Option<EmbedLink>,
    pub author_icon_url: Option<String>,
    pub footer_text: Option<String>,
    pub footer_icon_url: Option<String>,
    pub fields: Vec<EmbedField>,
    pub provider_name: Option<String>,
    pub provider_url: Option<EmbedLink>,
    pub thumbnail: Option<EmbedImage>,
    pub image: Option<EmbedImage>,
    /// A video or gifv: shown as its preview image, never played.
    pub is_video: bool,
    /// Set by `row()`: the body links to `url` inside a spoiler, so the card is covered as one
    /// unit (links inert, images not fetched, accessibility "Spoiler"). Always false when stored.
    pub spoiler: bool,
}

// Embed markdown parses (test builds only), so a test can show snapshots cause none.
#[cfg(test)]
thread_local! {
    pub(crate) static PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A message's `embeds` array; anything but an array gives none.
pub(crate) fn parse(v: Option<&Value>) -> Vec<Embed> {
    let Some(list) = v.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut budget = Budget(TOTAL_CHARS);
    list.iter()
        .filter(|e| e.is_object())
        .take(MAX_EMBEDS)
        .map(|e| embed(e, &mut budget))
        .collect()
}

/// Characters left of `TOTAL_CHARS`.
struct Budget(usize);

impl Budget {
    /// `s` cut to `cap` characters and then to what is left of the budget, which it uses up;
    /// also whether anything was cut.
    fn take(&mut self, s: &str, cap: usize) -> (String, bool) {
        let limit = cap.min(self.0);
        let end = s.char_indices().nth(limit).map_or(s.len(), |(i, _)| i);
        let kept = &s[..end];
        self.0 -= kept.chars().count();
        (kept.to_owned(), end < s.len())
    }

    /// A plain string field; `None` when absent or nothing of it is left.
    fn text(&mut self, v: Option<&Value>, cap: usize) -> Option<String> {
        let (kept, _) = self.take(v?.as_str()?, cap);
        (!kept.is_empty()).then_some(kept)
    }

    /// A markdown field, parsed once; a cut one holding `||` becomes one spoiler.
    fn markdown(&mut self, v: Option<&Value>, cap: usize) -> Vec<Block> {
        let Some(s) = v.and_then(Value::as_str) else {
            return Vec::new();
        };
        let (kept, cut) = self.take(s, cap);
        if kept.is_empty() {
            return Vec::new();
        }
        if cut && kept.contains("||") {
            return vec![Block::Paragraph(vec![Inline::Spoiler(vec![Inline::Text(
                kept,
            )])])];
        }
        #[cfg(test)]
        PARSES.with(|p| p.set(p.get() + 1));
        markdown::parse(&kept)
    }
}

/// `o.k`, when `o` is present.
fn at<'v>(o: Option<&'v Value>, k: &str) -> Option<&'v Value> {
    o?.get(k)
}

/// `o.k` as a string.
fn string<'v>(o: Option<&'v Value>, k: &str) -> Option<&'v str> {
    at(o, k)?.as_str()
}

fn embed(e: &Value, budget: &mut Budget) -> Embed {
    let author = e.get("author");
    let footer = e.get("footer");
    let provider = e.get("provider");
    // Display order: author name, title, description, fields, footer.
    let author_name = budget.text(at(author, "name"), AUTHOR_CHARS);
    let title = budget.text(e.get("title"), TITLE_CHARS);
    let description = budget.markdown(e.get("description"), DESCRIPTION_CHARS);
    let fields = e
        .get("fields")
        .and_then(Value::as_array)
        .map(|fs| {
            fs.iter()
                .filter(|f| f.is_object())
                .take(MAX_FIELDS)
                .filter_map(|f| {
                    let name = budget
                        .text(f.get("name"), FIELD_NAME_CHARS)
                        .unwrap_or_default();
                    let value = budget.markdown(f.get("value"), FIELD_VALUE_CHARS);
                    (!name.is_empty() || !value.is_empty()).then(|| EmbedField {
                        name,
                        value,
                        inline: f.get("inline").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let footer_text = budget.text(at(footer, "text"), FOOTER_CHARS);
    Embed {
        title,
        url: string(Some(e), "url").and_then(link),
        description,
        color: e
            .get("color")
            .and_then(Value::as_u64)
            .filter(|c| *c <= 0xFF_FFFF)
            .and_then(|c| u32::try_from(c).ok()),
        timestamp_ms: string(Some(e), "timestamp").and_then(timestamp_ms),
        author_name,
        author_url: string(author, "url").and_then(link),
        author_icon_url: string(author, "proxy_icon_url").and_then(media_url),
        footer_text,
        footer_icon_url: string(footer, "proxy_icon_url").and_then(media_url),
        fields,
        provider_name: string(provider, "name").map(str::to_owned),
        provider_url: string(provider, "url").and_then(link),
        thumbnail: image(e.get("thumbnail")),
        image: image(e.get("image")),
        is_video: matches!(
            e.get("type").and_then(Value::as_str),
            Some("video" | "gifv")
        ) || e.get("video").is_some_and(|v| !v.is_null()),
        spoiler: false,
    }
}

/// A link target: the message links' check (http(s), host, no userinfo, no C0/C1/Cf), at most
/// `LINK_BYTES`, and no explicit port (the plan's test list rejects `:8443`).
fn link(raw: &str) -> Option<EmbedLink> {
    if raw.len() > LINK_BYTES {
        return None;
    }
    let (url, host) = markdown::checked_link(raw)?;
    url::Url::parse(&url)
        .ok()?
        .port()
        .is_none()
        .then_some(EmbedLink { url, host })
}

/// Thumbnail or image: only `proxy_url` through `media_url`, with both sides in 1...16384.
fn image(v: Option<&Value>) -> Option<EmbedImage> {
    let v = v?;
    let side = |k: &str| {
        v.get(k)
            .and_then(Value::as_u64)
            .filter(|n| (1..=MAX_IMAGE_SIDE).contains(n))
            .and_then(|n| u32::try_from(n).ok())
    };
    Some(EmbedImage {
        url: media_url(v.get("proxy_url")?.as_str()?)?,
        width: side("width")?,
        height: side("height")?,
        description: v
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// `YYYY-MM-DDTHH:MM:SS(.d{1,9})?(Z|±HH:MM)` with a real date in years 1970-9999, as Unix
/// milliseconds (fractions past milliseconds dropped); anything else `None`.
pub(crate) fn timestamp_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    // The longest valid form: 19 + 10 (fraction) + 6 (offset).
    if b.len() > 35 {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let d = b.get(from..to)?;
        d.iter()
            .all(u8::is_ascii_digit)
            .then(|| d.iter().fold(0, |n, d| n * 10 + i64::from(d - b'0')))
    };
    for (i, c) in [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')] {
        if b.get(i) != Some(&c) {
            return None;
        }
    }
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let mut i = 19;
    let mut ms = 0;
    if b.get(i) == Some(&b'.') {
        let digits = b[i + 1..].iter().take_while(|d| d.is_ascii_digit()).count();
        if !(1..=9).contains(&digits) {
            return None;
        }
        ms = (0..3).fold(0, |n, k| {
            n * 10
                + b.get(i + 1 + k)
                    .filter(|_| k < digits)
                    .map_or(0, |d| i64::from(d - b'0'))
        });
        i += 1 + digits;
    }
    let offset_minutes = match b.get(i) {
        Some(b'Z') if b.len() == i + 1 => 0,
        Some(&sign @ (b'+' | b'-')) if b.len() == i + 6 && b[i + 3] == b':' => {
            let (h, m) = (num(i + 1, i + 3)?, num(i + 4, i + 6)?);
            if h > 23 || m > 59 {
                return None;
            }
            if sign == b'-' {
                -(h * 60 + m)
            } else {
                h * 60 + m
            }
        }
        _ => return None,
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=month_days).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    days_from_civil(year, month, day)
        .checked_mul(86_400_000)?
        .checked_add(((hour * 60 + minute) * 60 + second) * 1000 + ms)?
        .checked_sub(offset_minutes.checked_mul(60_000)?)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (the arithmetic of Howard Hinnant's
/// public-domain `days_from_civil` algorithm, written from its description).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
