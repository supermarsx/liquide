/// Platform-specific audio playback backends.
///
/// Each backend is gated behind `cfg(target_os)`. The public API is
/// `play_wav_bytes` (in-memory) and `play_wav_file` (on-disk path).

/// Result type for playback operations.
pub type PlayResult = Result<(), PlayError>;

/// Errors that can occur during sound playback.
#[derive(Debug)]
pub enum PlayError {
    /// No suitable audio backend was found on this platform.
    NoBackend,
    /// The external playback command failed.
    CommandFailed(String),
    /// I/O error (e.g. writing temp file).
    Io(std::io::Error),
}

impl std::fmt::Display for PlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayError::NoBackend => f.write_str("no audio playback backend available"),
            PlayError::CommandFailed(msg) => write!(f, "playback command failed: {}", msg),
            PlayError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for PlayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlayError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PlayError {
    fn from(e: std::io::Error) -> Self {
        PlayError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Linux playback
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::io::Write;

    /// Detected Linux audio backend, in order of preference.
    #[derive(Debug, Clone, Copy)]
    enum LinuxBackend {
        PwPlay, // PipeWire
        PaPlay, // PulseAudio
        APlay,  // ALSA
    }

    fn detect_backend() -> Option<LinuxBackend> {
        // Try PipeWire first, then PulseAudio, then ALSA.
        for (cmd, backend) in [
            ("pw-play", LinuxBackend::PwPlay),
            ("paplay", LinuxBackend::PaPlay),
            ("aplay", LinuxBackend::APlay),
        ] {
            if std::process::Command::new("which")
                .arg(cmd)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                return Some(backend);
            }
        }
        None
    }

    fn backend_command(backend: LinuxBackend) -> &'static str {
        match backend {
            LinuxBackend::PwPlay => "pw-play",
            LinuxBackend::PaPlay => "paplay",
            LinuxBackend::APlay => "aplay",
        }
    }

    pub fn play_wav_file(path: &str) -> PlayResult {
        let backend = detect_backend().ok_or(PlayError::NoBackend)?;
        let cmd = backend_command(backend);
        let status = std::process::Command::new(cmd)
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(PlayError::CommandFailed(format!(
                "{} exited with {}",
                cmd, status
            )))
        }
    }

    pub fn play_wav_bytes(data: &[u8]) -> PlayResult {
        let backend = detect_backend().ok_or(PlayError::NoBackend)?;
        let cmd = backend_command(backend);

        // Pipe WAV data to stdin. pw-play and paplay read from stdin
        // when given "-" as the file argument; aplay reads stdin by default.
        let stdin_arg = match backend {
            LinuxBackend::PwPlay | LinuxBackend::PaPlay => "-",
            LinuxBackend::APlay => "-",
        };

        let mut child = std::process::Command::new(cmd)
            .arg(stdin_arg)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(data);
        }
        // Drop stdin to signal EOF.
        drop(child.stdin.take());

        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(PlayError::CommandFailed(format!(
                "{} exited with {}",
                cmd, status
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// Windows playback
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    pub fn play_wav_file(path: &str) -> PlayResult {
        // Use PowerShell's System.Media.SoundPlayer which ships with
        // every Windows install since Vista.
        let script = format!(
            "(New-Object System.Media.SoundPlayer '{}').PlaySync()",
            path.replace('\'', "''")
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(PlayError::CommandFailed(
                "PowerShell SoundPlayer failed".into(),
            ))
        }
    }

    pub fn play_wav_bytes(data: &[u8]) -> PlayResult {
        // Write to a temp file and play it. SoundPlayer requires a file path.
        let tmp = std::env::temp_dir().join("liquide_sound_tmp.wav");
        std::fs::write(&tmp, data)?;
        let result = play_wav_file(tmp.to_str().unwrap_or(""));
        let _ = std::fs::remove_file(&tmp);
        result
    }
}

// ---------------------------------------------------------------------------
// macOS playback
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub fn play_wav_file(path: &str) -> PlayResult {
        let status = std::process::Command::new("afplay")
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(PlayError::CommandFailed(format!(
                "afplay exited with {}",
                status
            )))
        }
    }

    pub fn play_wav_bytes(data: &[u8]) -> PlayResult {
        let tmp = std::env::temp_dir().join("liquide_sound_tmp.wav");
        std::fs::write(&tmp, data)?;
        let result = play_wav_file(tmp.to_str().unwrap_or(""));
        let _ = std::fs::remove_file(&tmp);
        result
    }
}

// ---------------------------------------------------------------------------
// Public API (delegates to platform module)
// ---------------------------------------------------------------------------

/// Play a WAV file from disk. The path must point to a valid WAV file.
///
/// Uses the best available platform backend:
/// - **Linux**: PipeWire (`pw-play`) > PulseAudio (`paplay`) > ALSA (`aplay`)
/// - **Windows**: PowerShell `System.Media.SoundPlayer`
/// - **macOS**: `afplay`
pub fn play_wav_file(path: &str) -> PlayResult {
    #[cfg(target_os = "linux")]
    {
        return linux::play_wav_file(path);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::play_wav_file(path);
    }
    #[cfg(target_os = "macos")]
    {
        return macos::play_wav_file(path);
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = path;
        Err(PlayError::NoBackend)
    }
}

/// Play WAV audio data from an in-memory buffer.
///
/// On platforms that require a file path (Windows, macOS), this writes
/// to a temporary file, plays it, then cleans up.
pub fn play_wav_bytes(data: &[u8]) -> PlayResult {
    #[cfg(target_os = "linux")]
    {
        return linux::play_wav_bytes(data);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::play_wav_bytes(data);
    }
    #[cfg(target_os = "macos")]
    {
        return macos::play_wav_bytes(data);
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = data;
        Err(PlayError::NoBackend)
    }
}

/// Spawn a non-blocking playback of a WAV file. Returns immediately.
///
/// Errors from the spawned playback are silently ignored.
pub fn play_wav_file_async(path: &str) {
    let path = path.to_owned();
    std::thread::spawn(move || {
        let _ = play_wav_file(&path);
    });
}

/// Spawn a non-blocking playback of in-memory WAV data. Returns immediately.
pub fn play_wav_bytes_async(data: Vec<u8>) {
    std::thread::spawn(move || {
        let _ = play_wav_bytes(&data);
    });
}
