//! The one rule for image URLs that cross to the views (wave 3 E, Media URLs row; closes M2 F11).
//! Embed images, embed icons and image attachments are shown only from Discord's own media hosts.
//! Swift mirrors `MEDIA_HOSTS` in `MediaImages.hosts` and re-checks before every fetch.

/// Discord's media proxy and CDN hosts, compared by exact string with `url::Url::host_str()`
/// (which is lowercase ASCII for these). Mirrored in Swift as `MediaImages.hosts`.
pub const MEDIA_HOSTS: [&str; 4] = [
    "media.discordapp.net",
    "images-ext-1.discordapp.net",
    "images-ext-2.discordapp.net",
    "cdn.discordapp.com",
];

/// `raw` as a canonical URL when it is `https`, on one of `MEDIA_HOSTS`, with no explicit port
/// (the scheme default is normalised away), no user or password, and no C0, C1 or Unicode Cf
/// character anywhere in `raw`; otherwise `None`.
pub(crate) fn media_url(raw: &str) -> Option<String> {
    if raw.chars().any(crate::markdown::forbidden_in_url) {
        return None;
    }
    let url = url::Url::parse(raw).ok()?;
    let ok = url.scheme() == "https"
        && url.host_str().is_some_and(|h| MEDIA_HOSTS.contains(&h))
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none();
    ok.then(|| url.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_url_accepts_only_discord_media_hosts_over_https() {
        for host in MEDIA_HOSTS {
            let raw = format!("https://{host}/attachments/1/2/a.png?ex=1&hm=2");
            assert_eq!(media_url(&raw).as_deref(), Some(raw.as_str()), "{raw}");
        }
        // The url crate lowercases the host and drops the default port before the comparison.
        assert_eq!(
            media_url("https://MEDIA.discordapp.net/a.png").as_deref(),
            Some("https://media.discordapp.net/a.png")
        );
        assert_eq!(
            media_url("https://media.discordapp.net:443/a.png").as_deref(),
            Some("https://media.discordapp.net/a.png")
        );
        for raw in [
            "http://media.discordapp.net/a.png",
            "https://media.discordapp.net:8443/a.png",
            "https://user@media.discordapp.net/a.png",
            "https://user:pw@media.discordapp.net/a.png",
            "https://media.discordapp.net./a.png",
            "https://media.discordapp.net.evil.test/a.png",
            "https://evil.test\\@media.discordapp.net/",
            // Cyrillic "е" (U+0435): punycode, so not the host.
            "https://m\u{0435}dia.discordapp.net/a.png",
            "//media.discordapp.net/a.png",
            "file:///etc/passwd",
            "data:image/png;base64,AAAA",
            "",
            "https://media.discordapp.net/a\u{200B}.png",
            "https://media.discordapp.net/a\u{0085}.png",
            "https://media.discordapp.net/a\u{7}.png",
            "https://discordapp.net/a.png",
            "https://evil.test/https://media.discordapp.net/a.png",
        ] {
            assert_eq!(media_url(raw), None, "{raw:?}");
        }
    }
}
