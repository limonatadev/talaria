use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use opencv::core::Scalar;
use opencv::highgui;
use opencv::imgcodecs;
use opencv::imgproc;
use opencv::prelude::*;

use crate::camera::LatestFrameSlot;
use crate::types::{AppEvent, PreviewCommand, PreviewEvent};

pub fn spawn_preview_thread(
    cmd_rx: Receiver<PreviewCommand>,
    event_tx: Sender<AppEvent>,
    latest: Arc<LatestFrameSlot>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut enabled = false;
        let mut last_seq = 0;
        let window = "talaria-camera-preview";
        let image_window = "talaria-image-preview";
        let mut image_path: Option<PathBuf> = None;
        let mut image_mat: Option<Mat> = None;
        let mut image_loaded: Option<PathBuf> = None;

        loop {
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    PreviewCommand::SetEnabled(next) => {
                        enabled = next;
                        if !enabled {
                            let _ = highgui::destroy_window(window);
                        }
                    }
                    PreviewCommand::ShowImage(path) => {
                        image_path = path;
                        image_mat = None;
                        image_loaded = None;
                        if image_path.is_none() {
                            let _ = highgui::destroy_window(image_window);
                        }
                    }
                    PreviewCommand::Shutdown => {
                        let _ = highgui::destroy_window(window);
                        let _ = highgui::destroy_window(image_window);
                        return;
                    }
                }
            }

            let wants_preview = enabled || image_path.is_some();
            if !wants_preview {
                thread::sleep(Duration::from_millis(30));
                continue;
            }

            if std::env::var_os("DISPLAY").is_none() {
                let _ = event_tx.send(AppEvent::Preview(PreviewEvent::Unavailable(
                    "No DISPLAY set; preview window disabled.".to_string(),
                )));
                enabled = false;
                image_path = None;
                image_mat = None;
                image_loaded = None;
                continue;
            }

            if enabled {
                if let Some((seq, frame, size)) = latest.get_latest() {
                    if seq != last_seq {
                        if let Err(err) = render_frame(window, frame, size) {
                            let _ = event_tx
                                .send(AppEvent::Preview(PreviewEvent::Error(err.to_string())));
                            enabled = false;
                        } else {
                            last_seq = seq;
                        }
                    }
                } else if let Err(err) = render_placeholder(window) {
                    let _ = event_tx.send(AppEvent::Preview(PreviewEvent::Error(err.to_string())));
                    enabled = false;
                }
            }

            if let Some(path) = &image_path {
                let should_load = image_loaded.as_ref().map(|p| p != path).unwrap_or(true);
                if should_load {
                    match imgcodecs::imread(
                        path.to_string_lossy().as_ref(),
                        imgcodecs::IMREAD_COLOR,
                    ) {
                        Ok(mat) => {
                            image_mat = Some(mat);
                            image_loaded = Some(path.clone());
                        }
                        Err(err) => {
                            let _ = event_tx
                                .send(AppEvent::Preview(PreviewEvent::Error(err.to_string())));
                        }
                    }
                }
                if let Some(mat) = &image_mat {
                    if let Err(err) = highgui::imshow(image_window, mat) {
                        let _ =
                            event_tx.send(AppEvent::Preview(PreviewEvent::Error(err.to_string())));
                    }
                }
            }

            let _ = highgui::wait_key(1);
            thread::sleep(Duration::from_millis(5));
        }
    })
}

fn render_frame(window: &str, frame: Mat, size: Option<(i32, i32)>) -> opencv::Result<()> {
    let mut annotated = frame.clone();
    if let Some((w, h)) = size {
        let text = format!("{w}x{h}");
        imgproc::put_text(
            &mut annotated,
            &text,
            opencv::core::Point::new(12, 24),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.6,
            Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_AA,
            false,
        )?;
    } else {
        // TODO: overlay resolution and FPS once capture status is wired into preview.
    }

    // TODO: implement ROI selection via mouse callbacks.
    highgui::imshow(window, &annotated)?;
    Ok(())
}

fn render_placeholder(window: &str) -> opencv::Result<()> {
    let mut placeholder = Mat::zeros(480, 640, opencv::core::CV_8UC3)?.to_mat()?;
    imgproc::put_text(
        &mut placeholder,
        "No signal",
        opencv::core::Point::new(18, 40),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.8,
        Scalar::new(220.0, 220.0, 220.0, 0.0),
        2,
        imgproc::LINE_AA,
        false,
    )?;
    highgui::imshow(window, &placeholder)?;
    Ok(())
}
