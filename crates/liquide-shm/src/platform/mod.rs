#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::SharedMemory;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::SharedMemory;
