pub mod capture;
pub mod annotate;
pub mod output;

pub use capture::{ScreenCapture, CaptureMode, CaptureResult, CaptureRegion};
pub use annotate::{Annotation, AnnotationTool, AnnotationState};
pub use output::{OutputFormat, OutputTarget, save_screenshot};
