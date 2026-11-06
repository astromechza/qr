//! Camera discovery, permission handling and frame capture via AVFoundation (nokhwa).

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use image::GrayImage;
use nokhwa::Camera;
use nokhwa::pixel_format::LumaFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};

/// A discovered capture device.
#[derive(Debug, Clone)]
pub struct Device {
    pub index: CameraIndex,
    pub name: String,
    pub description: String,
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.index, self.name)?;
        if !self.description.is_empty() && self.description != self.name {
            write!(f, " ({})", self.description)?;
        }
        Ok(())
    }
}

/// Ensure the process has camera permission, prompting via the macOS TCC dialog if needed.
pub fn ensure_permission() -> Result<()> {
    if nokhwa::nokhwa_check() {
        return Ok(());
    }
    let (tx, rx) = mpsc::channel();
    nokhwa::nokhwa_initialize(move |granted| {
        let _ = tx.send(granted);
    });
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(true) => Ok(()),
        Ok(false) => bail!(
            "camera access denied; grant it to your terminal in System Settings > Privacy & Security > Camera"
        ),
        Err(_) => bail!("timed out waiting for camera permission"),
    }
}

pub fn list_devices() -> Result<Vec<Device>> {
    let infos = nokhwa::query(ApiBackend::AVFoundation).context("failed to enumerate cameras")?;
    Ok(infos
        .into_iter()
        .map(|i| Device {
            index: i.index().clone(),
            name: i.human_name(),
            description: i.description().to_string(),
        })
        .collect())
}

/// Resolve a user-supplied `--device` value: an exact index, or a case-insensitive
/// substring of the device name.
pub fn resolve<'a>(devices: &'a [Device], query: &str) -> Result<&'a Device> {
    if let Ok(n) = query.parse::<u32>() {
        return devices
            .iter()
            .find(|d| matches!(d.index, CameraIndex::Index(i) if i == n))
            .ok_or_else(|| anyhow!("no device with index {n}"));
    }
    let q = query.to_lowercase();
    let hits: Vec<&Device> = devices
        .iter()
        .filter(|d| d.name.to_lowercase().contains(&q))
        .collect();
    match hits.as_slice() {
        [one] => Ok(one),
        [] => bail!("no device matching {query:?}"),
        many => bail!(
            "{query:?} is ambiguous, matches: {}",
            many.iter()
                .map(|d| d.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// An open, streaming camera yielding grayscale frames.
pub struct Stream {
    camera: Camera,
}

impl Stream {
    pub fn open(device: &Device) -> Result<Self> {
        let format =
            RequestedFormat::new::<LumaFormat>(RequestedFormatType::AbsoluteHighestResolution);
        let mut camera = Camera::new(device.index.clone(), format)
            .with_context(|| format!("failed to open camera {}", device.name))?;
        camera
            .open_stream()
            .context("failed to start camera stream")?;
        Ok(Self { camera })
    }

    pub fn frame(&mut self) -> Result<GrayImage> {
        let buf = self.camera.frame().context("failed to capture frame")?;
        buf.decode_image::<LumaFormat>()
            .context("failed to decode frame")
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.camera.stop_stream();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(i: u32, name: &str) -> Device {
        Device {
            index: CameraIndex::Index(i),
            name: name.into(),
            description: String::new(),
        }
    }

    #[test]
    fn resolve_by_index_and_name() {
        let ds = vec![
            dev(0, "MacBook Pro Camera"),
            dev(1, "iPhone Camera"),
            dev(2, "OBS Virtual Camera"),
        ];
        assert_eq!(resolve(&ds, "1").unwrap().name, "iPhone Camera");
        assert_eq!(resolve(&ds, "macbook").unwrap().name, "MacBook Pro Camera");
        assert!(resolve(&ds, "7").is_err());
        assert!(resolve(&ds, "nope").is_err());
        let err = resolve(&ds, "camera").unwrap_err().to_string();
        assert!(err.contains("ambiguous"), "{err}");
    }
}
