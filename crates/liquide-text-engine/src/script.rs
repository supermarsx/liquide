//! Unicode script detection (UAX #24).
//!
//! Assigns a Unicode script to each character in a string and groups
//! contiguous characters of the same script into runs. `Common` and
//! `Inherited` scripts are resolved to the surrounding context.

use serde::{Deserialize, Serialize};

/// Unicode script identifier (subset covering the most common scripts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Script {
    Latin,
    Greek,
    Cyrillic,
    Arabic,
    Hebrew,
    Devanagari,
    Bengali,
    Thai,
    Han,
    Hiragana,
    Katakana,
    Hangul,
    Georgian,
    Armenian,
    Ethiopic,
    Tamil,
    Telugu,
    Kannada,
    Malayalam,
    Tibetan,
    Myanmar,
    Khmer,
    Lao,
    Common,
    Inherited,
    Unknown,
}

impl Script {
    /// Detect the script of a single Unicode code point.
    #[must_use]
    #[allow(unreachable_patterns)]
    pub fn from_char(ch: char) -> Self {
        let cp = ch as u32;
        match cp {
            // Basic Latin, Latin Extended, Latin Supplement
            0x0041..=0x024F | 0x1E00..=0x1EFF | 0x2C60..=0x2C7F | 0xA720..=0xA7FF => Self::Latin,
            // Greek and Coptic
            0x0370..=0x03FF | 0x1F00..=0x1FFF => Self::Greek,
            // Cyrillic
            0x0400..=0x04FF | 0x0500..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F => Self::Cyrillic,
            // Arabic
            0x0600..=0x06FF
            | 0x0750..=0x077F
            | 0x08A0..=0x08FF
            | 0xFB50..=0xFDFF
            | 0xFE70..=0xFEFF => Self::Arabic,
            // Hebrew
            0x0590..=0x05FF | 0xFB1D..=0xFB4F => Self::Hebrew,
            // Devanagari
            0x0900..=0x097F | 0xA8E0..=0xA8FF => Self::Devanagari,
            // Bengali
            0x0980..=0x09FF => Self::Bengali,
            // Tamil
            0x0B80..=0x0BFF => Self::Tamil,
            // Telugu
            0x0C00..=0x0C7F => Self::Telugu,
            // Kannada
            0x0C80..=0x0CFF => Self::Kannada,
            // Malayalam
            0x0D00..=0x0D7F => Self::Malayalam,
            // Thai
            0x0E00..=0x0E7F => Self::Thai,
            // Lao
            0x0E80..=0x0EFF => Self::Lao,
            // Tibetan
            0x0F00..=0x0FFF => Self::Tibetan,
            // Myanmar
            0x1000..=0x109F => Self::Myanmar,
            // Georgian
            0x10A0..=0x10FF | 0x2D00..=0x2D2F => Self::Georgian,
            // Armenian
            0x0530..=0x058F | 0xFB00..=0xFB17 => Self::Armenian,
            // Ethiopic
            0x1200..=0x137F | 0x1380..=0x139F => Self::Ethiopic,
            // Hangul
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => Self::Hangul,
            // Hiragana
            0x3040..=0x309F => Self::Hiragana,
            // Katakana
            0x30A0..=0x30FF | 0x31F0..=0x31FF => Self::Katakana,
            // CJK Unified Ideographs (Han)
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0xF900..=0xFAFF => Self::Han,
            // Khmer
            0x1780..=0x17FF | 0x19E0..=0x19FF => Self::Khmer,
            // Common punctuation, symbols, digits, whitespace
            0x0000..=0x0040
            | 0x005B..=0x0060
            | 0x007B..=0x00BF
            | 0x2000..=0x206F
            | 0x2070..=0x209F
            | 0x20A0..=0x20CF
            | 0x2100..=0x214F
            | 0x2190..=0x21FF
            | 0x2200..=0x22FF
            | 0x2300..=0x23FF
            | 0x2500..=0x257F
            | 0x2580..=0x259F
            | 0x25A0..=0x25FF
            | 0x2600..=0x26FF
            | 0x2700..=0x27BF
            | 0x3000..=0x303F
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF => Self::Common,
            // Combining marks
            0x0300..=0x036F | 0xFE20..=0xFE2F => Self::Inherited,
            _ => Self::Unknown,
        }
    }

    /// Whether this script uses right-to-left base direction.
    #[must_use]
    pub fn is_rtl(&self) -> bool {
        matches!(self, Self::Arabic | Self::Hebrew)
    }

    /// Whether this is a "weak" script (Common or Inherited) that should
    /// inherit direction from surrounding text.
    #[must_use]
    pub fn is_weak(&self) -> bool {
        matches!(self, Self::Common | Self::Inherited | Self::Unknown)
    }
}

/// A contiguous run of characters sharing the same resolved script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRun {
    /// Byte offset of the start of this run in the source string.
    pub start: usize,
    /// Byte offset of the end (exclusive).
    pub end: usize,
    /// The resolved script.
    pub script: Script,
}

/// Detects scripts and produces script runs from input text.
pub struct ScriptDetector;

impl ScriptDetector {
    /// Analyze a string and produce a list of script runs.
    ///
    /// `Common` and `Inherited` characters are merged into the surrounding
    /// script context. If no strong script is found, they remain `Common`.
    #[must_use]
    pub fn detect(text: &str) -> Vec<ScriptRun> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut runs = Vec::new();
        let mut current_script = Script::Common;
        let mut run_start = 0;

        for (byte_idx, ch) in text.char_indices() {
            let ch_script = Script::from_char(ch);
            let resolved = if ch_script.is_weak() {
                current_script // Inherit from context
            } else {
                ch_script
            };

            if resolved != current_script && !current_script.is_weak() && !resolved.is_weak() {
                // Script boundary — flush current run
                if byte_idx > run_start {
                    runs.push(ScriptRun {
                        start: run_start,
                        end: byte_idx,
                        script: current_script,
                    });
                }
                run_start = byte_idx;
                current_script = resolved;
            } else if current_script.is_weak() && !resolved.is_weak() {
                current_script = resolved;
            }
        }

        // Flush final run
        if run_start < text.len() {
            runs.push(ScriptRun {
                start: run_start,
                end: text.len(),
                script: current_script,
            });
        }

        // Second pass: resolve any remaining Common/Inherited runs by
        // looking at neighbors.
        for i in 0..runs.len() {
            if runs[i].script.is_weak() {
                // Look left then right for a strong script
                if i > 0 && !runs[i - 1].script.is_weak() {
                    runs[i].script = runs[i - 1].script;
                } else if i + 1 < runs.len() && !runs[i + 1].script.is_weak() {
                    runs[i].script = runs[i + 1].script;
                }
            }
        }

        // Merge adjacent runs with the same script
        let mut merged = Vec::new();
        for run in runs {
            if let Some(last) = merged.last_mut() {
                let last: &mut ScriptRun = last;
                if last.script == run.script && last.end == run.start {
                    last.end = run.end;
                    continue;
                }
            }
            merged.push(run);
        }

        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latin_detection() {
        assert_eq!(Script::from_char('A'), Script::Latin);
        assert_eq!(Script::from_char('z'), Script::Latin);
        assert_eq!(Script::from_char('é'), Script::Latin);
    }

    #[test]
    fn test_cjk_detection() {
        assert_eq!(Script::from_char('中'), Script::Han);
        assert_eq!(Script::from_char('あ'), Script::Hiragana);
        assert_eq!(Script::from_char('ア'), Script::Katakana);
        assert_eq!(Script::from_char('한'), Script::Hangul);
    }

    #[test]
    fn test_rtl_detection() {
        assert_eq!(Script::from_char('ع'), Script::Arabic);
        assert!(Script::Arabic.is_rtl());
        assert_eq!(Script::from_char('א'), Script::Hebrew);
        assert!(Script::Hebrew.is_rtl());
        assert!(!Script::Latin.is_rtl());
    }

    #[test]
    fn test_common_characters() {
        assert_eq!(Script::from_char(' '), Script::Common);
        assert_eq!(Script::from_char('1'), Script::Common);
        assert_eq!(Script::from_char('.'), Script::Common);
    }

    #[test]
    fn test_script_runs_pure_latin() {
        let runs = ScriptDetector::detect("Hello world");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].script, Script::Latin);
    }

    #[test]
    fn test_script_runs_mixed() {
        let runs = ScriptDetector::detect("Hello 你好");
        assert!(runs.len() >= 2);
        assert_eq!(runs[0].script, Script::Latin);
        assert_eq!(runs.last().unwrap().script, Script::Han);
    }

    #[test]
    fn test_empty_string() {
        let runs = ScriptDetector::detect("");
        assert!(runs.is_empty());
    }

    #[test]
    fn test_script_coverage() {
        assert_eq!(Script::from_char('Δ'), Script::Greek);
        assert_eq!(Script::from_char('Б'), Script::Cyrillic);
        assert_eq!(Script::from_char('ก'), Script::Thai);
        assert_eq!(Script::from_char('ᄀ'), Script::Hangul);
    }
}
