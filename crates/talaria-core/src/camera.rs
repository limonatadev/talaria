use crate::error::{Error, Result};
#[derive(Debug, Clone)]
pub struct CameraDevice {
    pub index: u32,
    pub description: String,
}

#[cfg(feature = "camera")]
mod impls {
    use super::{CameraDevice, Error, Result};
    use image::ImageBuffer;
    use nokhwa::Camera;
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{CameraIndex, FrameFormat, Resolution};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn list_devices() -> Result<Vec<CameraDevice>> {
        let devices = nokhwa::query_devices(
            nokhwa::native_api_backend()
                .map_err(|e| Error::CameraUnavailable(format!("camera backend error: {e}")))?,
        )
        .map_err(|e| Error::CameraUnavailable(format!("failed to list cameras: {e}")))?;

        Ok(devices
            .into_iter()
            .enumerate()
            .map(|(idx, dev)| CameraDevice {
                index: idx as u32,
                description: dev.human_name().unwrap_or_else(|_| "camera".into()),
            })
            .collect())
    }

    pub fn capture_one(device_idx: Option<u32>, out_path: &Path) -> Result<PathBuf> {
        let mut camera = open_camera(device_idx)?;
        let frame = camera
            .frame()
            .map_err(|e| Error::CameraUnavailable(format!("capture error: {e}")))?;
        let buffer = frame
            .decode_image::<ImageBuffer<image::Rgb<u8>, Vec<u8>>>(
                nokhwa::buffer::BufferFormat::Rgb(RgbFormat::RGBX),
            )
            .map_err(|e| Error::CameraUnavailable(format!("decode error: {e}")))?;

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| Error::CameraUnavailable(format!("failed creating dir: {e}")))?;
        }
        buffer
            .save(out_path)
            .map_err(|e| Error::CameraUnavailable(format!("save error: {e}")))?;
        Ok(out_path.to_path_buf())
    }

    pub fn capture_many(
        count: usize,
        device_idx: Option<u32>,
        out_dir: &Path,
    ) -> Result<Vec<PathBuf>> {
        let mut results = Vec::new();
        for i in 0..count {
            let filename = format!("capture-{}-{}.png", timestamp_ms(), i);
            let path = out_dir.join(filename);
            let path_captured = capture_one(device_idx, &path)?;
            results.push(path_captured);
        }
        Ok(results)
    }

    fn open_camera(device_idx: Option<u32>) -> Result<Camera> {
        let idx = device_idx.unwrap_or(0);
        let mut cam = Camera::new(
            CameraIndex::Index(idx as usize),
            Resolution::new(1280, 720),
            FrameFormat::MJPEG,
            30,
        )
        .map_err(|e| Error::CameraUnavailable(format!("open camera failed: {e}")))?;
        cam.open_stream()
            .map_err(|e| Error::CameraUnavailable(format!("open stream failed: {e}")))?;
        Ok(cam)
    }

    fn timestamp_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }
}

#[cfg(not(feature = "camera"))]
mod impls {
    use super::{CameraDevice, Error, Result};
    use std::path::{Path, PathBuf};

    pub fn list_devices() -> Result<Vec<CameraDevice>> {
        Err(Error::CameraUnavailable(
            "built without camera support; enable feature `camera`".into(),
        ))
    }

    pub fn capture_one(_device_idx: Option<u32>, _out_path: &Path) -> Result<PathBuf> {
        Err(Error::CameraUnavailable(
            "built without camera support; enable feature `camera`".into(),
        ))
    }

    pub fn capture_many(
        _count: usize,
        _device_idx: Option<u32>,
        _out_dir: &Path,
    ) -> Result<Vec<PathBuf>> {
        Err(Error::CameraUnavailable(
            "built without camera support; enable feature `camera`".into(),
        ))
    }
}

pub use impls::*;
