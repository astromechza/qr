//! Rendering decoded QR codes for humans (with clickable links) or as JSON lines.

use std::io::{self, IsTerminal, Write};

use crate::decode::Decoded;

pub struct Printer {
    json: bool,
    hyperlinks: bool,
}

impl Printer {
    pub fn new(json: bool) -> Self {
        Self {
            json,
            hyperlinks: io::stdout().is_terminal(),
        }
    }

    pub fn print(&self, d: &Decoded) -> io::Result<()> {
        let mut out = io::stdout().lock();
        if self.json {
            serde_json::to_writer(&mut out, d)?;
            writeln!(out)?;
        } else {
            out.write_all(render_text(d, self.hyperlinks).as_bytes())?;
        }
        out.flush()
    }
}

/// Wrap `url` in an OSC 8 escape so supporting terminals render it as a clickable link.
fn hyperlink(url: &str) -> String {
    // Strip control characters so payload content can't inject terminal escapes.
    let safe: String = url.chars().filter(|c| !c.is_control()).collect();
    format!("\x1b]8;;{safe}\x1b\\{safe}\x1b]8;;\x1b\\")
}

fn render_text(d: &Decoded, hyperlinks: bool) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "QR code: version {} | ECC {} | mask {} | {} bytes{}\n",
        d.version,
        d.ecc_level,
        d.mask,
        d.hex.as_ref().map_or(d.text.len(), |h| h.len() / 2),
        if d.utf8 { "" } else { " | binary" }
    ));
    s.push_str("--- data ---\n");
    // Escape control chars (other than newline/tab) to keep the terminal safe.
    for line in d.text.lines() {
        let line: String = line
            .chars()
            .flat_map(|c| {
                if c.is_control() && c != '\t' {
                    c.escape_default().collect::<Vec<_>>()
                } else {
                    vec![c]
                }
            })
            .collect();
        s.push_str(&line);
        s.push('\n');
    }
    if let Some(hex) = &d.hex {
        s.push_str("--- hex ---\n");
        s.push_str(hex);
        s.push('\n');
    }
    if !d.urls.is_empty() {
        s.push_str("--- urls ---\n");
        for u in &d.urls {
            if hyperlinks {
                s.push_str(&hyperlink(u));
            } else {
                s.push_str(u);
            }
            s.push('\n');
        }
    }
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Decoded {
        Decoded {
            version: 2,
            ecc_level: "M",
            mask: 3,
            text: "see https://x.example\x1b[31m".into(),
            utf8: true,
            hex: None,
            urls: vec!["https://x.example".into()],
        }
    }

    #[test]
    fn text_output_escapes_control_chars() {
        let out = render_text(&sample(), false);
        assert!(!out.contains('\x1b'), "{out:?}");
        assert!(out.contains("--- urls ---\nhttps://x.example\n"));
    }

    #[test]
    fn hyperlinks_use_osc8() {
        let out = render_text(&sample(), true);
        assert!(out.contains("\x1b]8;;https://x.example\x1b\\https://x.example\x1b]8;;\x1b\\"));
    }
}
