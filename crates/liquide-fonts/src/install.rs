//! Font installation and uninstallation.
//!
//! Handles installing font files from various sources: local files,
//! drag-and-drop, URLs, and Git repositories.

use std::path::{Path, PathBuf};

use crate::catalog::{FontEntry, FontSource};
use crate::error::{FontError, Result};

/// Supported font file extensions.
const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "woff", "woff2", "ttc"];

/// Install a font file from a local path.
pub fn install_from_path(
    source_path: &Path,
    install_dir: &Path,
    source: FontSource,
) -> Result<FontEntry> {
    // Validate the file extension.
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !FONT_EXTENSIONS.contains(&ext.as_str()) {
        return Err(FontError::InvalidFormat {
            path: source_path.display().to_string(),
        });
    }

    // Validate the file exists.
    if !source_path.exists() {
        return Err(FontError::NotFound {
            path: source_path.display().to_string(),
        });
    }

    // Create installation directory if needed.
    std::fs::create_dir_all(install_dir)?;

    // Copy the file.
    let file_name = source_path
        .file_name()
        .ok_or_else(|| FontError::InvalidFormat {
            path: source_path.display().to_string(),
        })?;
    let dest_path = install_dir.join(file_name);
    std::fs::copy(source_path, &dest_path)?;

    let file_size = std::fs::metadata(&dest_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let stem = dest_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    // Parse family and style from filename.
    let (family, style, weight, italic) = parse_font_name(stem);

    tracing::info!(
        family = %family,
        style = %style,
        path = %dest_path.display(),
        "font installed"
    );

    Ok(FontEntry {
        family,
        style,
        weight,
        italic,
        path: dest_path,
        format: ext,
        file_size,
        source,
        tags: Vec::new(),
        activated: true,
        glyph_count: 0,
        script_coverage: Vec::new(),
        version: String::new(),
        license: String::new(),
        designer: String::new(),
    })
}

/// Uninstall a font by removing its file from disk.
pub fn uninstall(entry: &FontEntry) -> Result<()> {
    if !entry.path.exists() {
        return Err(FontError::NotFound {
            path: entry.path.display().to_string(),
        });
    }

    // Don't allow uninstalling system fonts.
    if entry.source == FontSource::System {
        return Err(FontError::UninstallFailed {
            reason: "cannot uninstall system fonts".into(),
        });
    }

    std::fs::remove_file(&entry.path)?;
    tracing::info!(
        family = %entry.family,
        path = %entry.path.display(),
        "font uninstalled"
    );
    Ok(())
}

/// Validate a URL for font import safety.
pub fn validate_import_url(url: &str, allowed_domains: &[String]) -> Result<()> {
    // Parse the URL to extract the domain.
    let domain = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");

    if domain.is_empty() {
        return Err(FontError::UnsafeUrl {
            reason: "could not parse domain from URL".into(),
        });
    }

    // Reject http:// (require HTTPS).
    if url.starts_with("http://") {
        return Err(FontError::UnsafeUrl {
            reason: "only HTTPS URLs are allowed for font import".into(),
        });
    }

    // Check against allowed domains.
    let is_allowed = allowed_domains
        .iter()
        .any(|allowed| domain == allowed.as_str() || domain.ends_with(&format!(".{allowed}")));

    if !is_allowed {
        return Err(FontError::UnsafeUrl {
            reason: format!(
                "domain '{domain}' is not in the allowed list: {}",
                allowed_domains.join(", ")
            ),
        });
    }

    Ok(())
}

/// Validate a Git repository URL for font import.
pub fn validate_git_url(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        return Err(FontError::UnsafeUrl {
            reason: "only HTTPS git URLs are allowed".into(),
        });
    }

    // Must end in .git or be a known git hosting domain.
    let is_git = url.ends_with(".git")
        || url.contains("github.com")
        || url.contains("gitlab.com")
        || url.contains("bitbucket.org");

    if !is_git {
        return Err(FontError::GitError {
            reason: format!("URL does not appear to be a git repository: {url}"),
        });
    }

    Ok(())
}

/// Scan a directory for font files and return paths.
pub fn scan_directory(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut fonts = Vec::new();
    if !dir.exists() {
        return Ok(fonts);
    }

    scan_dir_recursive(dir, &mut fonts)?;
    fonts.sort();
    Ok(fonts)
}

fn scan_dir_recursive(dir: &Path, fonts: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir_recursive(&path, fonts)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if FONT_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                fonts.push(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_https_url_allowed() {
        let allowed = vec!["fonts.google.com".to_string(), "github.com".to_string()];
        assert!(validate_import_url("https://fonts.google.com/font.ttf", &allowed).is_ok());
    }

    #[test]
    fn reject_http_url() {
        let allowed = vec!["example.com".to_string()];
        let result = validate_import_url("http://example.com/font.ttf", &allowed);
        assert!(result.is_err());
    }

    #[test]
    fn reject_unlisted_domain() {
        let allowed = vec!["safe.com".to_string()];
        let result = validate_import_url("https://evil.com/font.ttf", &allowed);
        assert!(result.is_err());
    }

    #[test]
    fn validate_git_url_https() {
        assert!(validate_git_url("https://github.com/user/fonts.git").is_ok());
    }

    #[test]
    fn reject_git_url_http() {
        assert!(validate_git_url("http://github.com/user/fonts.git").is_err());
    }

    #[test]
    fn scan_empty_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let fonts = scan_directory(dir.path()).unwrap();
        assert!(fonts.is_empty());
    }

    #[test]
    fn scan_directory_with_fonts() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.ttf"), b"fake").unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"text").unwrap();

        let fonts = scan_directory(dir.path()).unwrap();
        assert_eq!(fonts.len(), 1);
        assert!(fonts[0].extension().unwrap() == "ttf");
    }

    #[test]
    fn scan_nonexistent_directory() {
        let fonts = scan_directory(Path::new("/nonexistent/fonts")).unwrap();
        assert!(fonts.is_empty());
    }
}

/// Parse a font family name, style, weight, and italic flag from a filename stem.
fn parse_font_name(stem: &str) -> (String, String, u16, bool) {
    // Common patterns: "FontFamily-Style", "FontFamily_Style", "FontFamilyStyle"
    let parts: Vec<&str> = stem.split(|c: char| c == '-' || c == '_').collect();

    if parts.len() >= 2 {
        let family = parts[0].to_string();
        let style_part = parts[1..].join(" ");
        let lower = style_part.to_lowercase();
        let italic = lower.contains("italic") || lower.contains("oblique");
        let weight = weight_from_style(&lower);
        (family, style_part, weight, italic)
    } else {
        (stem.to_string(), "Regular".into(), 400, false)
    }
}

fn weight_from_style(style: &str) -> u16 {
    if style.contains("thin") || style.contains("hairline") {
        100
    } else if style.contains("extralight") || style.contains("ultralight") {
        200
    } else if style.contains("light") {
        300
    } else if style.contains("medium") {
        500
    } else if style.contains("semibold") || style.contains("demibold") {
        600
    } else if style.contains("extrabold") || style.contains("ultrabold") {
        800
    } else if style.contains("bold") {
        700
    } else if style.contains("black") || style.contains("heavy") {
        900
    } else {
        400
    }
}
