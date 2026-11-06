//! End-to-end tests for `qr --file`.

use std::path::Path;
use std::process::{Command, Output};

use image::{GrayImage, Luma};
use qrcode::QrCode;

fn write_qr(dir: &Path, name: &str, data: &str) -> std::path::PathBuf {
    let img: GrayImage = QrCode::new(data.as_bytes())
        .unwrap()
        .render::<Luma<u8>>()
        .module_dimensions(8, 8)
        .build();
    let path = dir.join(name);
    img.save(&path).unwrap();
    path
}

fn qr(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qr"))
        .args(args)
        .output()
        .unwrap()
}

fn tmpdir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("qr-test-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn file_with_url_prints_data_and_urls() {
    let path = write_qr(&tmpdir(), "url.jpg", "Hello https://example.com/x");
    let out = qr(&["--file", path.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("--- data ---\nHello https://example.com/x\n"),
        "{stdout}"
    );
    // stdout is a pipe, so no OSC 8 escapes.
    assert!(
        stdout.contains("--- urls ---\nhttps://example.com/x\n"),
        "{stdout}"
    );
}

#[test]
fn file_json_output() {
    let path = write_qr(&tmpdir(), "json.png", "https://example.org");
    let out = qr(&["--json", "--file", path.to_str().unwrap()]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["text"], "https://example.org");
    assert_eq!(v["urls"][0], "https://example.org");
}

#[test]
fn file_without_qr_exits_1() {
    let path = tmpdir().join("blank.png");
    GrayImage::from_pixel(100, 100, Luma([255]))
        .save(&path)
        .unwrap();
    let out = qr(&["--file", path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn missing_file_exits_2() {
    let out = qr(&["--file", "/nonexistent/nope.png"]);
    assert_eq!(out.status.code(), Some(2));
}
