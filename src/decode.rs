//! Pure QR detection/decoding over an in-memory grayscale image.

use image::GrayImage;
use serde::Serialize;

/// A single successfully decoded QR code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decoded {
    pub version: usize,
    pub ecc_level: &'static str,
    pub mask: u16,
    /// Decoded payload as text (lossy if the payload is not valid UTF-8).
    pub text: String,
    /// True if `text` is an exact representation of the payload.
    pub utf8: bool,
    /// Raw payload bytes, lower-case hex. Only populated for non-UTF-8 payloads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    /// URLs found inside the payload.
    pub urls: Vec<String>,
}

/// Map rqrr's ecc_level (the raw 2-bit format field) to the standard letter.
fn ecc_name(level: u16) -> &'static str {
    match level {
        0 => "M",
        1 => "L",
        2 => "H",
        3 => "Q",
        _ => "?",
    }
}

/// Find and decode every QR code in the image. Grids that fail to decode are skipped.
pub fn decode_image(img: &GrayImage) -> Vec<Decoded> {
    let mut prepared = rqrr::PreparedImage::prepare(img.clone());
    prepared
        .detect_grids()
        .into_iter()
        .filter_map(|grid| {
            let mut bytes = Vec::new();
            let meta = grid.decode_to(&mut bytes).ok()?;
            Some(build(meta, bytes))
        })
        .collect()
}

fn build(meta: rqrr::MetaData, bytes: Vec<u8>) -> Decoded {
    let (text, utf8, hex) = match String::from_utf8(bytes) {
        Ok(s) => (s, true, None),
        Err(e) => {
            let bytes = e.into_bytes();
            let hex = bytes.iter().map(|b| format!("{b:02x}")).collect();
            (
                String::from_utf8_lossy(&bytes).into_owned(),
                false,
                Some(hex),
            )
        }
    };
    Decoded {
        version: meta.version.0,
        ecc_level: ecc_name(meta.ecc_level),
        mask: meta.mask,
        urls: extract_urls(&text),
        text,
        utf8,
        hex,
    }
}

/// Extract URLs from a payload. Recognises plain URLs anywhere in the text, plus
/// bare `mailto:` / `tel:` / custom-scheme payloads that make up the whole code
/// (e.g. `otpauth://...`, `WIFI:` is deliberately excluded as it is not a link).
pub fn extract_urls(text: &str) -> Vec<String> {
    let mut finder = linkify::LinkFinder::new();
    finder.kinds(&[linkify::LinkKind::Url]);
    finder.url_must_have_scheme(true);
    let mut urls: Vec<String> = finder.links(text).map(|l| l.as_str().to_string()).collect();

    let trimmed = text.trim();
    if urls.is_empty() && is_single_uri(trimmed) {
        urls.push(trimmed.to_string());
    }
    urls.dedup();
    urls
}

/// A whole-payload URI with a scheme linkify doesn't pick up (mailto:, tel:, sms:, ...).
fn is_single_uri(s: &str) -> bool {
    const SCHEMES: &[&str] = &["mailto:", "tel:", "sms:", "smsto:", "geo:"];
    let lower = s.to_ascii_lowercase();
    !s.contains(char::is_whitespace) && SCHEMES.iter().any(|p| lower.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;
    use qrcode::QrCode;

    fn render(data: &[u8]) -> GrayImage {
        QrCode::new(data)
            .unwrap()
            .render::<Luma<u8>>()
            .quiet_zone(true)
            .module_dimensions(6, 6)
            .build()
    }

    #[test]
    fn decodes_url_payload() {
        let out = decode_image(&render(b"https://example.com/path?q=1"));
        assert_eq!(out.len(), 1);
        let d = &out[0];
        assert_eq!(d.text, "https://example.com/path?q=1");
        assert!(d.utf8);
        assert_eq!(d.hex, None);
        assert_eq!(d.urls, vec!["https://example.com/path?q=1"]);
        assert_eq!(d.ecc_level, "M"); // qrcode crate default
    }

    #[test]
    fn decodes_non_utf8_payload_as_hex() {
        let out = decode_image(&render(&[0xff, 0x00, 0xfe, 0x41]));
        assert_eq!(out.len(), 1);
        assert!(!out[0].utf8);
        assert_eq!(out[0].hex.as_deref(), Some("ff00fe41"));
    }

    #[test]
    fn blank_image_yields_nothing() {
        let img = GrayImage::from_pixel(200, 200, Luma([255]));
        assert!(decode_image(&img).is_empty());
    }

    #[test]
    fn extracts_urls_embedded_in_text() {
        let urls = extract_urls("Visit https://a.example and http://b.example/x for info");
        assert_eq!(urls, vec!["https://a.example", "http://b.example/x"]);
    }

    #[test]
    fn extracts_whole_payload_uris() {
        assert_eq!(
            extract_urls("mailto:me@example.com"),
            vec!["mailto:me@example.com"]
        );
        assert_eq!(extract_urls("tel:+15551234"), vec!["tel:+15551234"]);
        assert!(extract_urls("WIFI:T:WPA;S:net;P:pw;;").is_empty());
        assert!(extract_urls("just some text").is_empty());
    }
}
