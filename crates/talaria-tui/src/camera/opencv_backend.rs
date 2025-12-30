use opencv::prelude::*;
use opencv::videoio::{CAP_ANY, VideoCapture};

pub fn open_device(index: i32) -> opencv::Result<VideoCapture> {
    VideoCapture::new(index, CAP_ANY)
}
