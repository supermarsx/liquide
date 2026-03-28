/// Supported audio file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundFormat {
    /// Waveform Audio File Format (uncompressed PCM).
    Wav,
    /// Ogg Vorbis compressed audio.
    Ogg,
    /// Free Lossless Audio Codec.
    Flac,
}

impl SoundFormat {
    /// Returns the conventional file extension (without dot).
    pub fn extension(&self) -> &'static str {
        match self {
            SoundFormat::Wav => "wav",
            SoundFormat::Ogg => "ogg",
            SoundFormat::Flac => "flac",
        }
    }

    /// Returns the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            SoundFormat::Wav => "audio/wav",
            SoundFormat::Ogg => "audio/ogg",
            SoundFormat::Flac => "audio/flac",
        }
    }

    /// Detect format from a file extension string (case-insensitive).
    pub fn from_extension(ext: &str) -> Option<SoundFormat> {
        match ext.to_ascii_lowercase().as_str() {
            "wav" | "wave" => Some(SoundFormat::Wav),
            "ogg" | "oga" => Some(SoundFormat::Ogg),
            "flac" => Some(SoundFormat::Flac),
            _ => None,
        }
    }

    /// Detect format from a file path by inspecting its extension.
    pub fn from_path(path: &str) -> Option<SoundFormat> {
        let ext = path.rsplit('.').next()?;
        SoundFormat::from_extension(ext)
    }
}

impl std::fmt::Display for SoundFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.extension())
    }
}

/// A reference to a sound file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundFile {
    /// Path to the audio file (absolute or relative to theme directory).
    pub path: String,
    /// Audio format of the file.
    pub format: SoundFormat,
}

impl SoundFile {
    /// Create a new SoundFile, auto-detecting format from the path extension.
    /// Falls back to WAV if the extension is unrecognized.
    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        let format = SoundFormat::from_path(&path).unwrap_or(SoundFormat::Wav);
        SoundFile { path, format }
    }

    /// Create a new SoundFile with an explicit format.
    pub fn with_format(path: impl Into<String>, format: SoundFormat) -> Self {
        SoundFile {
            path: path.into(),
            format,
        }
    }
}
