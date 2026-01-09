#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
mod opencv;

#[cfg(windows)]
pub use windows::spawn_preview_thread;
#[cfg(not(windows))]
pub use opencv::spawn_preview_thread;
