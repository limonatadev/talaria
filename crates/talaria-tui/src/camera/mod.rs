use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;

#[cfg(not(windows))]
use opencv::core::Mat;
#[cfg(not(windows))]
use opencv::imgcodecs;
#[cfg(not(windows))]
use opencv::prelude::*;
#[cfg(not(windows))]
use opencv::videoio::VideoCapture;

#[cfg(windows)]
use image::RgbImage;
#[cfg(windows)]
use nokhwa::pixel_format::RgbFormat;
#[cfg(windows)]
use nokhwa::utils::{
    ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType,
};
#[cfg(windows)]
use nokhwa::Camera;

use crate::types::{AppEvent, CaptureCommand, CaptureEvent, CaptureStatus, CapturedFrame};
use crate::util::fs::timestamped_capture_path;
use crate::util::sharpness::laplacian_variance;

#[cfg(not(windows))]
mod opencv_backend;

#[cfg(not(windows))]
pub type Frame = Mat;
#[cfg(windows)]
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

#[cfg(not(windows))]
fn frame_dimensions(frame: &Frame) -> (i32, i32) {
    (frame.cols(), frame.rows())
}

#[cfg(windows)]
fn frame_dimensions(frame: &Frame) -> (i32, i32) {
    (frame.width() as i32, frame.height() as i32)
}

#[cfg(windows)]
pub fn list_devices() -> Result<Vec<CameraDevice>> {
    let devices = nokhwa::query(ApiBackend::MediaFoundation).context("query cameras")?;
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

#[cfg(not(windows))]
pub fn list_devices() -> Result<Vec<CameraDevice>> {
    Err(anyhow::anyhow!(
        "device picker not supported on this platform"
    ))
}

#[cfg(not(windows))]
pub fn spawn_capture_thread(
    cmd_rx: Receiver<CaptureCommand>,
    event_tx: Sender<AppEvent>,
    latest: Arc<LatestFrameSlot>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut device_index = 0;
        let mut streaming = false;
        let mut capture: Option<VideoCapture> = None;
        let mut frame = Mat::default();
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
                    CaptureCommand::CaptureBurst { n } => {
                        match capture_burst(
                            &mut capture,
                            device_index,
                            &latest,
                            output_dir.as_deref(),
                            n,
                        ) {
                            Ok((best, frames)) => {
                                let _ = event_tx.send(AppEvent::Capture(
                                    CaptureEvent::BurstCompleted {
                                        best_path: best,
                                        frames,
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

            if streaming {
                if capture.is_none() {
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
            }

            if streaming {
                if let Some(cap) = capture.as_mut() {
                    match cap.read(&mut frame) {
                        Ok(true) => {
                            // TODO: apply ROI cropping when ROI selection is implemented.
                            let _ = latest.set(frame.clone());
                            fps_frames += 1;
                        }
                        Ok(false) => {
                            thread::sleep(Duration::from_millis(5));
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

#[cfg(not(windows))]
fn open_device(index: i32) -> opencv::Result<VideoCapture> {
    let cap = opencv_backend::open_device(index)?;
    let opened = cap.is_opened()?;
    if !opened {
        return Err(opencv::Error::new(
            opencv::core::StsError,
            format!("device {index} not opened"),
        ));
    }
    Ok(cap)
}

#[cfg(not(windows))]
fn capture_one(
    capture: &mut Option<VideoCapture>,
    device_index: i32,
    latest: &LatestFrameSlot,
    out_dir: Option<&std::path::Path>,
) -> Result<(String, chrono::DateTime<chrono::Local>, Option<f64>)> {
    let out_dir = out_dir.context("no active session (set output dir first)")?;
    if let Some((_, frame, _)) = latest.get_latest() {
        return save_frame(out_dir, &frame);
    }

    let temp = if let Some(cap) = capture {
        cap
    } else {
        capture.insert(open_device(device_index).context("open device for capture")?)
    };

    let mut frame = Mat::default();
    temp.read(&mut frame).context("read frame")?;
    save_frame(out_dir, &frame)
}

#[cfg(not(windows))]
fn capture_burst(
    capture: &mut Option<VideoCapture>,
    device_index: i32,
    latest: &LatestFrameSlot,
    out_dir: Option<&std::path::Path>,
    n: usize,
) -> Result<(String, Vec<CapturedFrame>)> {
    let out_dir = out_dir.context("no active session (set output dir first)")?;
    let mut frames = Vec::with_capacity(n);
    let mut best_score = None;
    let mut best_path = None;

    let temp = if let Some(cap) = capture {
        cap
    } else {
        capture.insert(open_device(device_index).context("open device for burst")?)
    };

    for _ in 0..n {
        let mut frame = Mat::default();
        if !temp.read(&mut frame).context("read frame")? {
            if let Some((_, fallback, _)) = latest.get_latest() {
                frame = fallback;
            }
        }

        let (path, created_at, score_opt) = save_frame(out_dir, &frame)?;
        let score = score_opt.unwrap_or(0.0);

        if best_score.map(|best| score > best).unwrap_or(true) {
            best_score = Some(score);
            best_path = Some(path.clone());
        }

        frames.push(CapturedFrame {
            path,
            created_at,
            sharpness_score: score_opt,
        });
    }

    let best_path = best_path.context("best frame missing")?;
    Ok((best_path, frames))
}

#[cfg(not(windows))]
fn save_frame(
    out_dir: &std::path::Path,
    frame: &Mat,
) -> Result<(String, chrono::DateTime<chrono::Local>, Option<f64>)> {
    std::fs::create_dir_all(out_dir).context("create output dir")?;
    let created_at = chrono::Local::now();
    let path = timestamped_capture_path(out_dir, "jpg")?;
    let path_str = path.to_string_lossy().to_string();
    let sharpness_score = laplacian_variance(frame).ok();
    imgcodecs::imwrite(&path_str, frame, &opencv::core::Vector::new()).context("write frame")?;
    Ok((path_str, created_at, sharpness_score))
}

#[cfg(windows)]
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
                    CaptureCommand::CaptureBurst { n } => {
                        match capture_burst(
                            &mut capture,
                            device_index,
                            &latest,
                            output_dir.as_deref(),
                            n,
                        ) {
                            Ok((best, frames)) => {
                                let _ = event_tx.send(AppEvent::Capture(
                                    CaptureEvent::BurstCompleted {
                                        best_path: best,
                                        frames,
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

            if streaming {
                if capture.is_none() {
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

#[cfg(windows)]
fn open_device(index: i32) -> Result<Camera> {
    let idx = index.max(0) as u32;
    let requested = RequestedFormat::new::<RgbFormat>(
        RequestedFormatType::AbsoluteHighestFrameRate,
    );
    let mut cam = Camera::new(CameraIndex::Index(idx), requested).context("open camera")?;
    cam.open_stream().context("open stream")?;
    Ok(cam)
}

#[cfg(windows)]
fn read_frame(cam: &mut Camera) -> Result<Frame> {
    let frame = cam.frame().context("read frame")?;
    let buffer = frame.decode_image::<RgbFormat>().context("decode frame")?;
    Ok(buffer)
}

#[cfg(windows)]
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

#[cfg(windows)]
fn capture_burst(
    capture: &mut Option<Camera>,
    device_index: i32,
    latest: &LatestFrameSlot,
    out_dir: Option<&std::path::Path>,
    n: usize,
) -> Result<(String, Vec<CapturedFrame>)> {
    let out_dir = out_dir.context("no active session (set output dir first)")?;
    let mut frames = Vec::with_capacity(n);
    let mut best_score = None;
    let mut best_path = None;

    let temp = if let Some(cam) = capture {
        cam
    } else {
        capture.insert(open_device(device_index).context("open device for burst")?)
    };

    for _ in 0..n {
        let frame = match read_frame(temp) {
            Ok(frame) => frame,
            Err(err) => {
                if let Some((_, fallback, _)) = latest.get_latest() {
                    fallback
                } else {
                    return Err(err);
                }
            }
        };

        let (path, created_at, score_opt) = save_frame(out_dir, &frame)?;
        let score = score_opt.unwrap_or(0.0);

        if best_score.map(|best| score > best).unwrap_or(true) {
            best_score = Some(score);
            best_path = Some(path.clone());
        }

        frames.push(CapturedFrame {
            path,
            created_at,
            sharpness_score: score_opt,
        });
    }

    let best_path = best_path.context("best frame missing")?;
    Ok((best_path, frames))
}

#[cfg(windows)]
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
