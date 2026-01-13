use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use image::RgbImage;
use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
    Resolution,
};
use parking_lot::Mutex;

use crate::types::{AppEvent, CaptureCommand, CaptureEvent, CaptureStatus};
use crate::util::fs::timestamped_capture_path;
use crate::util::sharpness::laplacian_variance;

pub type Frame = RgbImage;

#[derive(Debug, Clone)]
pub struct CameraDevice {
    pub index: i32,
    pub name: String,
}

pub struct LatestFrameSlot {
    inner: Mutex<LatestFrame>,
}

struct LatestFrame {
    frame: Option<Frame>,
    seq: u64,
    dropped: u64,
    size: Option<(i32, i32)>,
}

impl LatestFrameSlot {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    pub fn new() -> Self {
        Self {
            inner: Mutex::new(LatestFrame {
                frame: None,
                seq: 0,
                dropped: 0,
                size: None,
            }),
        }
    }

    pub fn set(&self, frame: Frame) -> (u64, u64, Option<(i32, i32)>) {
        let mut inner = self.inner.lock();
        if inner.frame.is_some() {
            inner.dropped += 1;
        }
        inner.seq += 1;
        inner.size = Some(frame_dimensions(&frame));
        inner.frame = Some(frame);
        (inner.seq, inner.dropped, inner.size)
    }

    pub fn get_latest(&self) -> Option<(u64, Frame, Option<(i32, i32)>)> {
        let inner = self.inner.lock();
        inner
            .frame
            .as_ref()
            .map(|frame| (inner.seq, frame.clone(), inner.size))
    }

    pub fn dropped(&self) -> u64 {
        self.inner.lock().dropped
    }

    pub fn frame_size(&self) -> Option<(i32, i32)> {
        self.inner.lock().size
    }
}

fn frame_dimensions(frame: &Frame) -> (i32, i32) {
    (frame.width() as i32, frame.height() as i32)
}

pub fn list_devices() -> Result<Vec<CameraDevice>> {
    let devices = nokhwa::query(preferred_backend()).context("query cameras")?;
    let mut results = Vec::with_capacity(devices.len());
    for (fallback_idx, dev) in devices.into_iter().enumerate() {
        let mut name = dev.human_name();
        if name.trim().is_empty() {
            name = "camera".to_string();
        }
        let index = match dev.index().clone() {
            CameraIndex::Index(i) => i as i32,
            CameraIndex::String(id) => {
                results.push(CameraDevice {
                    index: fallback_idx as i32,
                    name: format!("{name} ({id})"),
                });
                continue;
            }
        };
        results.push(CameraDevice { index, name });
    }
    Ok(results)
}

pub fn spawn_capture_thread(
    cmd_rx: Receiver<CaptureCommand>,
    event_tx: Sender<AppEvent>,
    latest: Arc<LatestFrameSlot>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut device_index = 0;
        let mut streaming = false;
        let mut capture: Option<Camera> = None;
        let mut output_dir: Option<std::path::PathBuf> = None;
        let mut fps_last = Instant::now();
        let mut fps_frames = 0u32;
        let mut status_last = Instant::now();

        loop {
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    CaptureCommand::StartStream => {
                        if capture.is_none() {
                            match open_device(device_index) {
                                Ok(cap) => {
                                    capture = Some(cap);
                                    streaming = true;
                                }
                                Err(err) => {
                                    let _ = event_tx.send(AppEvent::Capture(CaptureEvent::Error(
                                        format!("open device {device_index}: {err}"),
                                    )));
                                    streaming = false;
                                }
                            }
                        } else {
                            streaming = true;
                        }
                    }
                    CaptureCommand::StopStream => {
                        streaming = false;
                    }
                    CaptureCommand::SetDevice { index } => {
                        device_index = index.max(0);
                        if streaming {
                            capture = None;
                            match open_device(device_index) {
                                Ok(cap) => capture = Some(cap),
                                Err(err) => {
                                    let _ = event_tx.send(AppEvent::Capture(CaptureEvent::Error(
                                        format!("open device {device_index}: {err}"),
                                    )));
                                    streaming = false;
                                }
                            }
                        }
                    }
                    CaptureCommand::SetOutputDir(dir) => {
                        output_dir = Some(dir);
                    }
                    CaptureCommand::ClearOutputDir => {
                        output_dir = None;
                    }
                    CaptureCommand::CaptureOne => {
                        match capture_one(
                            &mut capture,
                            device_index,
                            &latest,
                            output_dir.as_deref(),
                        ) {
                            Ok((path, created_at, sharpness_score)) => {
                                let _ = event_tx.send(AppEvent::Capture(
                                    CaptureEvent::CaptureCompleted {
                                        path,
                                        created_at,
                                        sharpness_score,
                                    },
                                ));
                            }
                            Err(err) => {
                                let _ = event_tx
                                    .send(AppEvent::Capture(CaptureEvent::Error(err.to_string())));
                            }
                        }
                    }
                    CaptureCommand::Shutdown => return,
                }
            }

            if streaming && capture.is_none() {
                match open_device(device_index) {
                    Ok(cap) => capture = Some(cap),
                    Err(err) => {
                        let _ = event_tx.send(AppEvent::Capture(CaptureEvent::Error(format!(
                            "open device {device_index}: {err}"
                        ))));
                        streaming = false;
                    }
                }
            }

            if streaming {
                if let Some(cam) = capture.as_mut() {
                    match read_frame(cam) {
                        Ok(frame) => {
                            let _ = latest.set(frame);
                            fps_frames += 1;
                        }
                        Err(err) => {
                            let _ = event_tx
                                .send(AppEvent::Capture(CaptureEvent::Error(err.to_string())));
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(10));
            }

            if status_last.elapsed() >= Duration::from_millis(500) {
                let elapsed = fps_last.elapsed().as_secs_f32().max(0.001);
                let fps = fps_frames as f32 / elapsed;
                fps_last = Instant::now();
                fps_frames = 0;

                let status = CaptureStatus {
                    streaming,
                    device_index,
                    fps,
                    dropped_frames: latest.dropped(),
                    frame_size: latest.frame_size(),
                };
                let _ = event_tx.send(AppEvent::Capture(CaptureEvent::Status(status)));
                status_last = Instant::now();
            }
        }
    })
}

fn preferred_backend() -> ApiBackend {
    #[cfg(target_os = "windows")]
    {
        return ApiBackend::MediaFoundation;
    }
    #[cfg(target_os = "linux")]
    {
        return ApiBackend::Video4Linux;
    }
    #[cfg(target_os = "macos")]
    {
        return ApiBackend::AVFoundation;
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        return ApiBackend::Auto;
    }
}

fn open_device(index: i32) -> Result<Camera> {
    let idx = index.max(0) as u32;
    let mut candidates = Vec::new();

    if let Some((w, h)) = preferred_resolution() {
        candidates.push(RequestedFormat::new::<RgbFormat>(
            RequestedFormatType::Closest(CameraFormat::new(
                Resolution::new(w, h),
                FrameFormat::MJPEG,
                30,
            )),
        ));
        candidates.push(RequestedFormat::new::<RgbFormat>(
            RequestedFormatType::Closest(CameraFormat::new(
                Resolution::new(w, h),
                FrameFormat::YUYV,
                30,
            )),
        ));
    }

    candidates.extend([
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(1280, 720),
            FrameFormat::MJPEG,
            30,
        ))),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(640, 480),
            FrameFormat::MJPEG,
            30,
        ))),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(1280, 704),
            FrameFormat::YUYV,
            30,
        ))),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(640, 480),
            FrameFormat::YUYV,
            30,
        ))),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate),
    ]);

    let mut last_err = None;
    for requested in candidates {
        match open_and_probe(idx, requested) {
            Ok(cam) => return Ok(cam),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("open camera failed")))
}

fn preferred_resolution() -> Option<(u32, u32)> {
    let raw = std::env::var("TALARIA_CAMERA_RESOLUTION")
        .ok()
        .or_else(|| std::env::var("TALARIA_CAMERA_RES").ok())?;
    let mut parts = raw.split(|c| c == 'x' || c == 'X');
    let w = parts.next()?.trim().parse::<u32>().ok()?;
    let h = parts.next()?.trim().parse::<u32>().ok()?;
    if parts.next().is_some() || w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn open_and_probe(index: u32, requested: RequestedFormat) -> Result<Camera> {
    let mut cam = Camera::with_backend(CameraIndex::Index(index), requested, preferred_backend())
        .context("open camera")?;
    cam.open_stream().context("open stream")?;
    let frame = cam.frame().context("probe frame")?;
    frame.decode_image::<RgbFormat>().context("probe decode")?;
    Ok(cam)
}

fn read_frame(cam: &mut Camera) -> Result<Frame> {
    let frame = cam.frame().context("read frame")?;
    let buffer = frame.decode_image::<RgbFormat>().context("decode frame")?;
    Ok(buffer)
}

fn capture_one(
    capture: &mut Option<Camera>,
    device_index: i32,
    latest: &LatestFrameSlot,
    out_dir: Option<&std::path::Path>,
) -> Result<(String, chrono::DateTime<chrono::Local>, Option<f64>)> {
    let out_dir = out_dir.context("no active session (set output dir first)")?;
    if let Some((_, frame, _)) = latest.get_latest() {
        return save_frame(out_dir, &frame);
    }

    let temp = if let Some(cam) = capture {
        cam
    } else {
        capture.insert(open_device(device_index).context("open device for capture")?)
    };

    let frame = read_frame(temp)?;
    save_frame(out_dir, &frame)
}

fn save_frame(
    out_dir: &std::path::Path,
    frame: &Frame,
) -> Result<(String, chrono::DateTime<chrono::Local>, Option<f64>)> {
    std::fs::create_dir_all(out_dir).context("create output dir")?;
    let created_at = chrono::Local::now();
    let path = timestamped_capture_path(out_dir, "jpg")?;
    let path_str = path.to_string_lossy().to_string();
    let sharpness_score = laplacian_variance(frame).ok();
    frame.save(&path).context("write frame")?;
    Ok((path_str, created_at, sharpness_score))
}
