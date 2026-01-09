use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use image::{Rgb, RgbImage};
use minifb::{Window, WindowOptions};

use crate::camera::LatestFrameSlot;
use crate::types::{AppEvent, PreviewCommand, PreviewEvent};

const CAMERA_WINDOW: &str = "talaria-camera-preview";
const IMAGE_WINDOW: &str = "talaria-image-preview";

struct WindowState {
    window: Window,
    size: (usize, usize),
    buffer: Vec<u32>,
}

impl WindowState {
    fn new(title: &str, size: (usize, usize)) -> Result<Self, String> {
        let mut options = WindowOptions::default();
        options.resize = true;
        let window = Window::new(title, size.0, size.1, options).map_err(|e| e.to_string())?;
        Ok(Self {
            window,
            size,
            buffer: vec![0; size.0 * size.1],
        })
    }

    fn update_frame(&mut self, frame: &RgbImage) -> Result<(), String> {
        let (width, height) = frame.dimensions();
        let size = (width as usize, height as usize);
        if self.size != size {
            return Err("frame size changed".to_string());
        }
        fill_buffer(frame, &mut self.buffer);
        self.window
            .update_with_buffer(&self.buffer, self.size.0, self.size.1)
            .map_err(|e| e.to_string())
    }

    fn refresh(&mut self) -> Result<(), String> {
        if self.buffer.is_empty() {
            self.window.update().map_err(|e| e.to_string())
        } else {
            self.window
                .update_with_buffer(&self.buffer, self.size.0, self.size.1)
                .map_err(|e| e.to_string())
        }
    }
}

pub fn spawn_preview_thread(
    cmd_rx: Receiver<PreviewCommand>,
    event_tx: Sender<AppEvent>,
    latest: Arc<LatestFrameSlot>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut enabled = false;
        let mut last_seq = 0;
        let mut image_path: Option<PathBuf> = None;
        let mut image_loaded: Option<PathBuf> = None;
        let mut image_buffer: Option<RgbImage> = None;
        let mut camera_window: Option<WindowState> = None;
        let mut image_window: Option<WindowState> = None;
        let placeholder = RgbImage::from_pixel(640, 480, Rgb([12, 12, 12]));

        loop {
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    PreviewCommand::SetEnabled(next) => {
                        enabled = next;
                        if !enabled {
                            camera_window = None;
                        }
                    }
                    PreviewCommand::ShowImage(path) => {
                        image_path = path;
                        image_buffer = None;
                        image_loaded = None;
                        if image_path.is_none() {
                            image_window = None;
                        }
                    }
                    PreviewCommand::Shutdown => return,
                }
            }

            let wants_preview = enabled || image_path.is_some();
            if !wants_preview {
                thread::sleep(Duration::from_millis(30));
                continue;
            }

            if enabled {
                let frame = if let Some((seq, frame, _)) = latest.get_latest() {
                    if seq != last_seq {
                        last_seq = seq;
                    }
                    Some(frame)
                } else {
                    None
                };

                if let Err(err) = render_frame(
                    CAMERA_WINDOW,
                    &mut camera_window,
                    frame.as_ref().unwrap_or(&placeholder),
                ) {
                    let _ = event_tx
                        .send(AppEvent::Preview(PreviewEvent::Error(err.to_string())));
                    enabled = false;
                    camera_window = None;
                }
            }

            if let Some(path) = &image_path {
                let should_load = image_loaded.as_ref().map(|p| p != path).unwrap_or(true);
                if should_load {
                    match image::open(path) {
                        Ok(img) => {
                            image_buffer = Some(img.to_rgb8());
                            image_loaded = Some(path.clone());
                        }
                        Err(err) => {
                            let _ = event_tx.send(AppEvent::Preview(PreviewEvent::Error(
                                err.to_string(),
                            )));
                        }
                    }
                }
                if let Some(img) = &image_buffer {
                    if let Err(err) = render_frame(IMAGE_WINDOW, &mut image_window, img) {
                        let _ = event_tx
                            .send(AppEvent::Preview(PreviewEvent::Error(err.to_string())));
                        image_window = None;
                    }
                }
            }

            if let Some(window) = camera_window.as_mut() {
                if !window.window.is_open() {
                    enabled = false;
                    camera_window = None;
                    let _ = event_tx.send(AppEvent::Preview(PreviewEvent::Unavailable(
                        "Preview window closed.".to_string(),
                    )));
                } else {
                    let _ = window.refresh();
                }
            }

            if let Some(window) = image_window.as_mut() {
                if !window.window.is_open() {
                    image_window = None;
                } else {
                    let _ = window.refresh();
                }
            }

            thread::sleep(Duration::from_millis(5));
        }
    })
}

fn render_frame(
    title: &str,
    window: &mut Option<WindowState>,
    frame: &RgbImage,
) -> Result<(), String> {
    let size = (frame.width() as usize, frame.height() as usize);
    let needs_new = match window {
        Some(win) => !win.window.is_open() || win.size != size,
        None => true,
    };
    if needs_new {
        *window = Some(WindowState::new(title, size)?);
    }

    if let Some(win) = window.as_mut() {
        if let Err(err) = win.update_frame(frame) {
            if err.contains("frame size changed") {
                *window = Some(WindowState::new(title, size)?);
                if let Some(win) = window.as_mut() {
                    win.update_frame(frame)?;
                }
            } else {
                return Err(err);
            }
        }
    }

    Ok(())
}

fn fill_buffer(frame: &RgbImage, buffer: &mut Vec<u32>) {
    let (width, height) = frame.dimensions();
    let len = (width * height) as usize;
    buffer.resize(len, 0);
    for (idx, pixel) in frame.pixels().enumerate() {
        let [r, g, b] = pixel.0;
        buffer[idx] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
    }
}
