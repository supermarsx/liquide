//! VT sequence parser.
//!
//! Parses ANSI/VT102/VT220/xterm escape sequences from a byte stream
//! and produces structured actions that the grid engine can apply.

use serde::{Deserialize, Serialize};

/// A parsed terminal action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Print a character at the cursor position.
    Print(char),
    /// Execute a C0 control code.
    Execute(u8),
    /// CSI sequence: cursor movement, erase, SGR, etc.
    CsiDispatch(CsiAction),
    /// OSC sequence: title, hyperlink, shell integration.
    OscDispatch(OscAction),
    /// ESC sequence (non-CSI/OSC).
    EscDispatch(u8),
}

/// CSI (Control Sequence Introducer) actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsiAction {
    /// Cursor Up (CUU).
    CursorUp(u32),
    /// Cursor Down (CUD).
    CursorDown(u32),
    /// Cursor Forward (CUF).
    CursorForward(u32),
    /// Cursor Back (CUB).
    CursorBack(u32),
    /// Cursor Position (CUP).
    CursorPosition { row: u32, col: u32 },
    /// Erase in Display (ED).
    EraseDisplay(EraseMode),
    /// Erase in Line (EL).
    EraseLine(EraseMode),
    /// Select Graphic Rendition (SGR).
    Sgr(Vec<SgrParam>),
    /// Scroll Up (SU).
    ScrollUp(u32),
    /// Scroll Down (SD).
    ScrollDown(u32),
    /// Set scrolling region (DECSTBM).
    SetScrollRegion { top: u32, bottom: u32 },
    /// Insert Lines (IL).
    InsertLines(u32),
    /// Delete Lines (DL).
    DeleteLines(u32),
    /// Insert Characters (ICH).
    InsertChars(u32),
    /// Delete Characters (DCH).
    DeleteChars(u32),
    /// Device Status Report (DSR).
    DeviceStatusReport,
    /// Unknown/unparsed CSI sequence.
    Unknown(Vec<u8>),
}

/// Erase mode for ED and EL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EraseMode {
    /// Erase from cursor to end.
    ToEnd,
    /// Erase from beginning to cursor.
    ToBeginning,
    /// Erase entire line/screen.
    All,
}

/// SGR (Select Graphic Rendition) parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgrParam {
    /// Reset all attributes.
    Reset,
    /// Bold / bright.
    Bold,
    /// Dim / faint.
    Dim,
    /// Italic.
    Italic,
    /// Underline.
    Underline,
    /// Blink.
    Blink,
    /// Reverse video.
    Reverse,
    /// Hidden / invisible.
    Hidden,
    /// Strikethrough.
    Strikethrough,
    /// Set foreground color (0-255 palette index).
    Foreground(u8),
    /// Set background color (0-255 palette index).
    Background(u8),
    /// True-color foreground.
    ForegroundRgb(u8, u8, u8),
    /// True-color background.
    BackgroundRgb(u8, u8, u8),
    /// Default foreground.
    DefaultForeground,
    /// Default background.
    DefaultBackground,
}

/// OSC (Operating System Command) actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscAction {
    /// Set window title (OSC 0 / OSC 2).
    SetTitle(String),
    /// Set working directory (OSC 7).
    SetWorkingDirectory(String),
    /// Shell integration: command start (OSC 133;A).
    CommandStart,
    /// Shell integration: command end with status (OSC 133;D).
    CommandEnd(Option<i32>),
    /// Hyperlink (OSC 8).
    Hyperlink { url: String, id: Option<String> },
    /// Unknown OSC.
    Unknown(String),
}

/// VT sequence parser state machine.
pub struct Parser {
    state: ParserState,
    params: Vec<u8>,
    osc_buf: String,
    intermediate: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Ground,
    Escape,
    CsiEntry,
    CsiParam,
    OscString,
}

impl Parser {
    /// Create a new parser.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            osc_buf: String::new(),
            intermediate: Vec::new(),
        }
    }

    /// Feed bytes into the parser and collect produced actions.
    pub fn feed(&mut self, input: &[u8], actions: &mut Vec<Action>) {
        for &byte in input {
            self.advance(byte, actions);
        }
    }

    fn advance(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match self.state {
            ParserState::Ground => self.ground(byte, actions),
            ParserState::Escape => self.escape(byte, actions),
            ParserState::CsiEntry | ParserState::CsiParam => self.csi(byte, actions),
            ParserState::OscString => self.osc(byte, actions),
        }
    }

    fn ground(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match byte {
            0x1b => {
                self.state = ParserState::Escape;
            }
            0x00..=0x1a | 0x1c..=0x1f => {
                actions.push(Action::Execute(byte));
            }
            _ => {
                if byte.is_ascii() {
                    actions.push(Action::Print(byte as char));
                }
            }
        }
    }

    fn escape(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match byte {
            b'[' => {
                self.state = ParserState::CsiEntry;
                self.params.clear();
                self.intermediate.clear();
            }
            b']' => {
                self.state = ParserState::OscString;
                self.osc_buf.clear();
            }
            _ => {
                actions.push(Action::EscDispatch(byte));
                self.state = ParserState::Ground;
            }
        }
    }

    fn csi(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match byte {
            b'0'..=b'9' | b';' => {
                self.params.push(byte);
                self.state = ParserState::CsiParam;
            }
            b' '..=b'/' => {
                self.intermediate.push(byte);
            }
            0x40..=0x7e => {
                let action = self.dispatch_csi(byte);
                actions.push(Action::CsiDispatch(action));
                self.state = ParserState::Ground;
            }
            _ => {
                self.state = ParserState::Ground;
            }
        }
    }

    fn osc(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match byte {
            0x07 => {
                let action = self.dispatch_osc();
                actions.push(Action::OscDispatch(action));
                self.state = ParserState::Ground;
            }
            0x1b => {
                let action = self.dispatch_osc();
                actions.push(Action::OscDispatch(action));
                self.state = ParserState::Ground;
            }
            _ => {
                if byte.is_ascii() {
                    self.osc_buf.push(byte as char);
                }
            }
        }
    }

    fn parse_params(&self) -> Vec<u32> {
        if self.params.is_empty() {
            return vec![];
        }
        let s = String::from_utf8_lossy(&self.params);
        s.split(';')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect()
    }

    fn dispatch_csi(&self, final_byte: u8) -> CsiAction {
        let params = self.parse_params();
        let p0 = params.first().copied().unwrap_or(1).max(1);

        match final_byte {
            b'A' => CsiAction::CursorUp(p0),
            b'B' => CsiAction::CursorDown(p0),
            b'C' => CsiAction::CursorForward(p0),
            b'D' => CsiAction::CursorBack(p0),
            b'H' | b'f' => {
                let row = params.first().copied().unwrap_or(1).max(1);
                let col = params.get(1).copied().unwrap_or(1).max(1);
                CsiAction::CursorPosition { row, col }
            }
            b'J' => {
                let mode = match params.first().copied().unwrap_or(0) {
                    1 => EraseMode::ToBeginning,
                    2 => EraseMode::All,
                    _ => EraseMode::ToEnd,
                };
                CsiAction::EraseDisplay(mode)
            }
            b'K' => {
                let mode = match params.first().copied().unwrap_or(0) {
                    1 => EraseMode::ToBeginning,
                    2 => EraseMode::All,
                    _ => EraseMode::ToEnd,
                };
                CsiAction::EraseLine(mode)
            }
            b'm' => CsiAction::Sgr(self.parse_sgr(&params)),
            b'S' => CsiAction::ScrollUp(p0),
            b'T' => CsiAction::ScrollDown(p0),
            b'L' => CsiAction::InsertLines(p0),
            b'M' => CsiAction::DeleteLines(p0),
            b'@' => CsiAction::InsertChars(p0),
            b'P' => CsiAction::DeleteChars(p0),
            b'r' => {
                let top = params.first().copied().unwrap_or(1);
                let bottom = params.get(1).copied().unwrap_or(0);
                CsiAction::SetScrollRegion { top, bottom }
            }
            b'n' if params.first() == Some(&6) => CsiAction::DeviceStatusReport,
            _ => CsiAction::Unknown(self.params.clone()),
        }
    }

    fn parse_sgr(&self, params: &[u32]) -> Vec<SgrParam> {
        if params.is_empty() {
            return vec![SgrParam::Reset];
        }
        let mut result = Vec::new();
        let mut i = 0;
        while i < params.len() {
            let p = params[i];
            match p {
                0 => result.push(SgrParam::Reset),
                1 => result.push(SgrParam::Bold),
                2 => result.push(SgrParam::Dim),
                3 => result.push(SgrParam::Italic),
                4 => result.push(SgrParam::Underline),
                5 => result.push(SgrParam::Blink),
                7 => result.push(SgrParam::Reverse),
                8 => result.push(SgrParam::Hidden),
                9 => result.push(SgrParam::Strikethrough),
                30..=37 => result.push(SgrParam::Foreground((p - 30) as u8)),
                38 if params.get(i + 1) == Some(&5) => {
                    if let Some(&idx) = params.get(i + 2) {
                        result.push(SgrParam::Foreground(idx as u8));
                        i += 2;
                    }
                }
                38 if params.get(i + 1) == Some(&2) => {
                    if let (Some(&r), Some(&g), Some(&b)) =
                        (params.get(i + 2), params.get(i + 3), params.get(i + 4))
                    {
                        result.push(SgrParam::ForegroundRgb(r as u8, g as u8, b as u8));
                        i += 4;
                    }
                }
                39 => result.push(SgrParam::DefaultForeground),
                40..=47 => result.push(SgrParam::Background((p - 40) as u8)),
                48 if params.get(i + 1) == Some(&5) => {
                    if let Some(&idx) = params.get(i + 2) {
                        result.push(SgrParam::Background(idx as u8));
                        i += 2;
                    }
                }
                48 if params.get(i + 1) == Some(&2) => {
                    if let (Some(&r), Some(&g), Some(&b)) =
                        (params.get(i + 2), params.get(i + 3), params.get(i + 4))
                    {
                        result.push(SgrParam::BackgroundRgb(r as u8, g as u8, b as u8));
                        i += 4;
                    }
                }
                49 => result.push(SgrParam::DefaultBackground),
                90..=97 => result.push(SgrParam::Foreground((p - 90 + 8) as u8)),
                100..=107 => result.push(SgrParam::Background((p - 100 + 8) as u8)),
                _ => {}
            }
            i += 1;
        }
        result
    }

    fn dispatch_osc(&self) -> OscAction {
        let buf = &self.osc_buf;
        if let Some(rest) = buf.strip_prefix("0;").or_else(|| buf.strip_prefix("2;")) {
            return OscAction::SetTitle(rest.to_string());
        }
        if let Some(rest) = buf.strip_prefix("7;") {
            return OscAction::SetWorkingDirectory(rest.to_string());
        }
        if buf == "133;A" {
            return OscAction::CommandStart;
        }
        if let Some(rest) = buf.strip_prefix("133;D;") {
            let code = rest.parse::<i32>().ok();
            return OscAction::CommandEnd(code);
        }
        if buf == "133;D" {
            return OscAction::CommandEnd(None);
        }
        if let Some(rest) = buf.strip_prefix("8;") {
            let parts: Vec<&str> = rest.splitn(2, ';').collect();
            if parts.len() == 2 {
                let id = if parts[0].is_empty() {
                    None
                } else {
                    Some(parts[0].to_string())
                };
                return OscAction::Hyperlink {
                    url: parts[1].to_string(),
                    id,
                };
            }
        }
        OscAction::Unknown(buf.to_string())
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}
