# qr

> **Note:** This project is _intentionally_ vibe coded with [Claude](https://claude.com/claude-code). It exists to provide a
> simple, local-only tool for reading QR codes from the macOS terminal — no network calls, no apps, no phone.

Wait for a QR code to appear on a webcam (or in an image file), then print its contents and any URLs as clickable
terminal links.

## Install

Requires macOS and a Rust toolchain.

```bash
cargo install --path .
```

## Usage

```bash
qr                      # discover cameras; prompts if several and stdin is a terminal
qr --device 0           # camera by index...
qr --device facetime    # ...or case-insensitive name substring
qr --file ./image.jpg   # decode an image instead
qr --list-devices       # list cameras
```

| Flag | Description |
|------|-------------|
| `-f, --file <FILE>` | Decode an image file (png, jpg, gif, webp, ...) |
| `-d, --device <DEVICE>` | Camera index or name substring |
| `-l, --list-devices` | List capture devices and exit |
| `-t, --timeout <SECS>` | Give up scanning after this many seconds |
| `-a, --all` | Keep scanning, printing each new distinct code |
| `--json` | One JSON object per code instead of text |

Exit codes: `0` code found, `1` no code found, `2` error.

The first camera scan triggers the macOS camera permission prompt for your terminal app. If it was denied, re-enable it
in **System Settings → Privacy & Security → Camera**.

## Output

For each code: version, ECC level, mask and size; the decoded text (control characters escaped); a hex dump for binary
payloads; and any URLs found (`http(s)://`, plus whole-payload `mailto:`, `tel:`, `sms:`, `geo:`). URLs are emitted as
OSC 8 hyperlinks when stdout is a terminal, so they're clickable in iTerm2, WezTerm, Ghostty, kitty and recent
Terminal.app.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
