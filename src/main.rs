mod camera;
mod decode;
mod output;

use std::collections::HashSet;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Parser;

use camera::Device;
use output::Printer;

/// Wait for a QR code from a camera or image and print its contents and URLs.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Decode an image file instead of using a camera.
    #[arg(short, long, conflicts_with_all = ["device", "list_devices"])]
    file: Option<PathBuf>,

    /// Camera to use: index or (case-insensitive) name substring.
    #[arg(short, long)]
    device: Option<String>,

    /// List available capture devices and exit.
    #[arg(short, long)]
    list_devices: bool,

    /// Give up after this many seconds when scanning a camera.
    #[arg(short, long, value_name = "SECS")]
    timeout: Option<u64>,

    /// Keep scanning and print every new distinct code instead of exiting after the first.
    #[arg(short, long)]
    all: bool,

    /// Output one JSON object per code instead of human-readable text.
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

/// Returns Ok(true) if at least one code was decoded.
fn run(cli: Cli) -> Result<bool> {
    let printer = Printer::new(cli.json);

    if let Some(path) = &cli.file {
        return scan_file(path, &printer);
    }

    let devices = camera::list_devices()?;

    if cli.list_devices {
        for d in &devices {
            println!("{d}");
        }
        return Ok(true);
    }

    let device = choose_device(&devices, cli.device.as_deref())?;
    camera::ensure_permission()?;
    scan_camera(&device, &cli, &printer)
}

fn scan_file(path: &PathBuf, printer: &Printer) -> Result<bool> {
    let img = image::open(path)
        .with_context(|| format!("failed to open image {}", path.display()))?
        .into_luma8();
    let codes = decode::decode_image(&img);
    if codes.is_empty() {
        eprintln!("no QR code found in {}", path.display());
        return Ok(false);
    }
    for c in &codes {
        printer.print(c)?;
    }
    Ok(true)
}

fn choose_device(devices: &[Device], query: Option<&str>) -> Result<Device> {
    if let Some(q) = query {
        return camera::resolve(devices, q).cloned();
    }
    match devices {
        [] => bail!("no capture devices found (use --file to scan an image)"),
        [one] => Ok(one.clone()),
        many if io::stdin().is_terminal() && io::stderr().is_terminal() => {
            let idx = dialoguer::Select::new()
                .with_prompt("Select a capture device")
                .items(many.iter().map(|d| d.to_string()))
                .default(0)
                .interact()
                .context("device selection cancelled")?;
            Ok(many[idx].clone())
        }
        many => bail!(
            "multiple capture devices found, pass --device:\n{}",
            many.iter()
                .map(|d| format!("  {d}"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}

fn scan_camera(device: &Device, cli: &Cli, printer: &Printer) -> Result<bool> {
    let mut stream = camera::Stream::open(device)?;
    eprintln!("scanning {} for QR codes (Ctrl-C to stop)...", device.name);

    let deadline = cli.timeout.map(|s| Instant::now() + Duration::from_secs(s));
    let mut seen = HashSet::new();
    loop {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            if seen.is_empty() {
                eprintln!("timed out without finding a QR code");
            }
            return Ok(!seen.is_empty());
        }
        let frame = stream.frame()?;
        for code in decode::decode_image(&frame) {
            if seen.insert(code.text.clone()) {
                printer.print(&code)?;
                if !cli.all {
                    return Ok(true);
                }
            }
        }
    }
}
