#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoiRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone)]
pub struct CaptureStatus {
    pub streaming: bool,
    pub device_index: i32,
    pub fps: f32,
    pub dropped_frames: u64,
    pub frame_size: Option<(i32, i32)>,
}

#[derive(Debug, Clone)]
pub enum CaptureCommand {
    StartStream,
    StopStream,
    SetDevice { index: i32 },
    CaptureOne,
    CaptureBurst { n: usize },
    SetRoi(Option<RoiRect>),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    Status(CaptureStatus),
    Error(String),
    CaptureCompleted {
        path: String,
    },
    BurstCompleted {
        best_path: String,
        all_paths: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub enum PreviewCommand {
    SetEnabled(bool),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum PreviewEvent {
    RoiSelected(RoiRect),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    Capture(CaptureEvent),
    Preview(PreviewEvent),
}
