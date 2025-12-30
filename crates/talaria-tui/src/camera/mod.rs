use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;

use opencv::core::Mat;
use opencv::imgcodecs;
use opencv::prelude::*;
use opencv::videoio::VideoCapture;

use crate::types::{CaptureCommand, CaptureEvent, CaptureStatus, UiEvent};
use crate::util::fs::timestamped_capture_path;
use crate::util::sharpness::laplacian_variance;

mod opencv_backend;

pub struct LatestFrameSlot {
    inner: Mutex<LatestFrame>,
}

struct LatestFrame {
    frame: Option<Mat>,
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

    pub fn set(&self, frame: Mat) -> (u64, u64, Option<(i32, i32)>) {
        let mut inner = self.inner.lock();
        if inner.frame.is_some() {
            inner.dropped += 1;
        }
        inner.seq += 1;
        inner.size = Some((frame.cols(), frame.rows()));
        inner.frame = Some(frame);
        (inner.seq, inner.dropped, inner.size)
    }

    pub fn get_latest(&self) -> Option<(u64, Mat, Option<(i32, i32)>)> {
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

pub fn spawn_capture_thread(
    cmd_rx: Receiver<CaptureCommand>,
    ui_tx: Sender<UiEvent>,
    latest: Arc<LatestFrameSlot>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut device_index = 0;
        let mut streaming = false;
        let mut capture: Option<VideoCapture> = None;
        let mut frame = Mat::default();
        let mut fps_last = Instant::now();
        let mut fps_frames = 0u32;
        let mut status_last = Instant::now();

        let mut roi = None;

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
                                    let _ = ui_tx.send(UiEvent::Capture(CaptureEvent::Error(
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
                                    let _ = ui_tx.send(UiEvent::Capture(CaptureEvent::Error(
                                        format!("open device {device_index}: {err}"),
                                    )));
                                    streaming = false;
                                }
                            }
                        }
                    }
                    CaptureCommand::CaptureOne => {
                        match capture_one(&mut capture, device_index, &latest) {
                            Ok(path) => {
                                let _ =
                                    ui_tx.send(UiEvent::Capture(CaptureEvent::CaptureCompleted {
                                        path,
                                    }));
                            }
                            Err(err) => {
                                let _ = ui_tx
                                    .send(UiEvent::Capture(CaptureEvent::Error(err.to_string())));
                            }
                        }
                    }
                    CaptureCommand::CaptureBurst { n } => {
                        match capture_burst(&mut capture, device_index, &latest, n) {
                            Ok((best, all)) => {
                                let _ =
                                    ui_tx.send(UiEvent::Capture(CaptureEvent::BurstCompleted {
                                        best_path: best,
                                        all_paths: all,
                                    }));
                            }
                            Err(err) => {
                                let _ = ui_tx
                                    .send(UiEvent::Capture(CaptureEvent::Error(err.to_string())));
                            }
                        }
                    }
                    CaptureCommand::SetRoi(next_roi) => {
                        roi = next_roi;
                    }
                    CaptureCommand::Shutdown => return,
                }
            }

            if streaming {
                if capture.is_none() {
                    match open_device(device_index) {
                        Ok(cap) => capture = Some(cap),
                        Err(err) => {
                            let _ = ui_tx.send(UiEvent::Capture(CaptureEvent::Error(format!(
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
                            let _ =
                                ui_tx.send(UiEvent::Capture(CaptureEvent::Error(err.to_string())));
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
                let _ = ui_tx.send(UiEvent::Capture(CaptureEvent::Status(status)));
                status_last = Instant::now();
            }

            let _ = roi;
        }
    })
}

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

fn capture_one(
    capture: &mut Option<VideoCapture>,
    device_index: i32,
    latest: &LatestFrameSlot,
) -> Result<String> {
    if let Some((_, frame, _)) = latest.get_latest() {
        return save_frame(&frame);
    }

    let temp = if let Some(cap) = capture {
        cap
    } else {
        capture.insert(open_device(device_index).context("open device for capture")?)
    };

    let mut frame = Mat::default();
    temp.read(&mut frame).context("read frame")?;
    save_frame(&frame)
}

fn capture_burst(
    capture: &mut Option<VideoCapture>,
    device_index: i32,
    latest: &LatestFrameSlot,
    n: usize,
) -> Result<(String, Vec<String>)> {
    let mut paths = Vec::with_capacity(n);
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

        let path = save_frame(&frame)?;
        let score = laplacian_variance(&frame).unwrap_or(0.0);

        if best_score.map(|best| score > best).unwrap_or(true) {
            best_score = Some(score);
            best_path = Some(path.clone());
        }

        paths.push(path);
    }

    let best_path = best_path.context("best frame missing")?;
    Ok((best_path, paths))
}

fn save_frame(frame: &Mat) -> Result<String> {
    let path = timestamped_capture_path("jpg")?;
    let path_str = path.to_string_lossy().to_string();
    imgcodecs::imwrite(&path_str, frame, &opencv::core::Vector::new()).context("write frame")?;
    Ok(path_str)
}
