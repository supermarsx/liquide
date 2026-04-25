pub mod annotate;
pub mod capture;
pub mod output;

pub use annotate::{Annotation, AnnotationState, AnnotationTool};
pub use capture::{CaptureMode, CaptureRegion, CaptureResult, ScreenCapture};
pub use output::{OutputFormat, OutputTarget, save_screenshot};
