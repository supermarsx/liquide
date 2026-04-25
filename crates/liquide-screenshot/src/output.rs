use crate::capture::CaptureResult;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Bmp,
    Ppm, // simple format we can write without deps
}

#[derive(Debug, Clone)]
pub enum OutputTarget {
    File(PathBuf),
    Clipboard,
    Both(PathBuf),
}

/// Save a screenshot to file (BMP or PPM format — no external deps needed)
pub fn save_screenshot(
    capture: &CaptureResult,
    path: &Path,
    format: OutputFormat,
) -> std::io::Result<()> {
    match format {
        OutputFormat::Bmp => save_bmp(capture, path),
        OutputFormat::Ppm => save_ppm(capture, path),
        OutputFormat::Png => {
            // PNG requires zlib compression — save as BMP as fallback
            // Full PNG support would need a PNG encoder
            save_bmp(capture, path)
        }
    }
}

fn save_bmp(capture: &CaptureResult, path: &Path) -> std::io::Result<()> {
    let w = capture.width;
    let h = capture.height;
    let row_size = ((w * 3 + 3) / 4) * 4; // BMP rows are 4-byte aligned
    let pixel_data_size = row_size * h;
    let file_size = 54 + pixel_data_size;

    let mut file = std::fs::File::create(path)?;

    // BMP Header (14 bytes)
    file.write_all(b"BM")?;
    file.write_all(&(file_size as u32).to_le_bytes())?;
    file.write_all(&0u16.to_le_bytes())?; // reserved
    file.write_all(&0u16.to_le_bytes())?; // reserved
    file.write_all(&54u32.to_le_bytes())?; // pixel data offset

    // DIB Header (40 bytes - BITMAPINFOHEADER)
    file.write_all(&40u32.to_le_bytes())?;
    file.write_all(&(w as i32).to_le_bytes())?;
    file.write_all(&(h as i32).to_le_bytes())?; // positive = bottom-up
    file.write_all(&1u16.to_le_bytes())?; // planes
    file.write_all(&24u16.to_le_bytes())?; // bits per pixel (BGR, no alpha for compat)
    file.write_all(&0u32.to_le_bytes())?; // compression (none)
    file.write_all(&(pixel_data_size as u32).to_le_bytes())?;
    file.write_all(&2835u32.to_le_bytes())?; // h resolution (72 DPI)
    file.write_all(&2835u32.to_le_bytes())?; // v resolution
    file.write_all(&0u32.to_le_bytes())?; // colors in palette
    file.write_all(&0u32.to_le_bytes())?; // important colors

    // Pixel data (bottom-up, BGR)
    let mut row_buf = vec![0u8; row_size as usize];
    for y in (0..h).rev() {
        for x in 0..w {
            let src_offset = (y * capture.stride + x * 4) as usize;
            let dst_offset = (x * 3) as usize;
            if src_offset + 2 < capture.pixels.len() && dst_offset + 2 < row_buf.len() {
                // Source is BGRA, BMP wants BGR
                row_buf[dst_offset] = capture.pixels[src_offset]; // B
                row_buf[dst_offset + 1] = capture.pixels[src_offset + 1]; // G
                row_buf[dst_offset + 2] = capture.pixels[src_offset + 2]; // R
            }
        }
        file.write_all(&row_buf)?;
    }

    Ok(())
}

fn save_ppm(capture: &CaptureResult, path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    // PPM header
    writeln!(file, "P6")?;
    writeln!(file, "{} {}", capture.width, capture.height)?;
    writeln!(file, "255")?;

    // Pixel data (RGB, top-down)
    for y in 0..capture.height {
        for x in 0..capture.width {
            let offset = (y * capture.stride + x * 4) as usize;
            if offset + 2 < capture.pixels.len() {
                // BGRA → RGB
                file.write_all(&[
                    capture.pixels[offset + 2],
                    capture.pixels[offset + 1],
                    capture.pixels[offset],
                ])?;
            } else {
                file.write_all(&[0, 0, 0])?;
            }
        }
    }

    Ok(())
}

/// Generate default screenshot filename
pub fn default_filename(format: OutputFormat) -> String {
    let ext = match format {
        OutputFormat::Png => "png",
        OutputFormat::Bmp => "bmp",
        OutputFormat::Ppm => "ppm",
    };

    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    format!("screenshot_{}.{}", secs, ext)
}

/// Get default screenshot directory
pub fn default_directory() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let pictures = PathBuf::from(profile).join("Pictures").join("Screenshots");
            if pictures.exists() || std::fs::create_dir_all(&pictures).is_ok() {
                return pictures;
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let pictures = PathBuf::from(home).join("Pictures").join("Screenshots");
            if pictures.exists() || std::fs::create_dir_all(&pictures).is_ok() {
                return pictures;
            }
        }
    }
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureRegion, CaptureResult};

    fn make_test_capture(w: u32, h: u32) -> CaptureResult {
        let stride = w * 4;
        let mut pixels = vec![0u8; (stride * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let off = (y * stride + x * 4) as usize;
                pixels[off] = 0; // B
                pixels[off + 1] = 128; // G
                pixels[off + 2] = 255; // R
                pixels[off + 3] = 255; // A
            }
        }
        CaptureResult {
            width: w,
            height: h,
            stride,
            pixels,
            region: CaptureRegion {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            timestamp: 1000,
        }
    }

    #[test]
    fn bmp_header_structure() {
        let cap = make_test_capture(2, 2);
        let dir = std::env::temp_dir();
        let path = dir.join("test_screenshot_header.bmp");
        save_screenshot(&cap, &path, OutputFormat::Bmp).unwrap();

        let data = std::fs::read(&path).unwrap();
        // BMP magic
        assert_eq!(&data[0..2], b"BM");
        // Pixel data offset = 54
        let offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);
        assert_eq!(offset, 54);
        // DIB header size = 40
        let dib_size = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
        assert_eq!(dib_size, 40);
        // Width = 2
        let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]);
        assert_eq!(width, 2);
        // Height = 2
        let height = i32::from_le_bytes([data[22], data[23], data[24], data[25]]);
        assert_eq!(height, 2);
        // Bits per pixel = 24
        let bpp = u16::from_le_bytes([data[28], data[29]]);
        assert_eq!(bpp, 24);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn ppm_roundtrip() {
        let cap = make_test_capture(3, 2);
        let dir = std::env::temp_dir();
        let path = dir.join("test_screenshot_roundtrip.ppm");
        save_screenshot(&cap, &path, OutputFormat::Ppm).unwrap();

        let data = std::fs::read(&path).unwrap();
        // PPM starts with "P6\n"
        assert!(data.starts_with(b"P6\n"));
        // Should contain "3 2\n"
        let header = String::from_utf8_lossy(&data[..20]);
        assert!(header.contains("3 2"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn default_filename_contains_timestamp_and_extension() {
        let name_bmp = default_filename(OutputFormat::Bmp);
        assert!(name_bmp.starts_with("screenshot_"));
        assert!(name_bmp.ends_with(".bmp"));

        let name_ppm = default_filename(OutputFormat::Ppm);
        assert!(name_ppm.ends_with(".ppm"));

        let name_png = default_filename(OutputFormat::Png);
        assert!(name_png.ends_with(".png"));

        // Timestamp portion should be numeric
        let ts_part = name_bmp
            .strip_prefix("screenshot_")
            .unwrap()
            .strip_suffix(".bmp")
            .unwrap();
        assert!(ts_part.parse::<u64>().is_ok());
    }

    #[test]
    fn default_directory_returns_existing_path() {
        let dir = default_directory();
        assert!(dir.exists());
    }
}
