use bitflags::bitflags;

bitflags! {
    /// Internal window state flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowFlags: u32 {
        /// Window is visible (shown).
        const VISIBLE           = 1 << 0;
        /// Window accepts input.
        const ENABLED           = 1 << 1;
        /// Window is minimized (iconic).
        const MINIMIZED         = 1 << 2;
        /// Window is maximized.
        const MAXIMIZED         = 1 << 3;
        /// Has invalid (dirty) region needing repaint.
        const UPDATE_DIRTY      = 1 << 4;
        /// Needs non-client area repaint.
        const SEND_NC_PAINT     = 1 << 5;
        /// Frame is drawn (window is active/focused).
        const FRAME_ON          = 1 << 6;
        /// Window is being destroyed.
        const IN_DESTROY        = 1 << 7;
        /// Composited / layered window.
        const LAYERED           = 1 << 8;
        /// Click-through (transparent to hit testing).
        const TRANSPARENT       = 1 << 9;
        /// Always-on-top.
        const TOPMOST           = 1 << 10;
    }
}

bitflags! {
    /// Window style flags (analogous to WS_* styles).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowStyle: u32 {
        /// Overlapped (normal) window.
        const OVERLAPPED        = 0;
        /// Popup window (no parent frame).
        const POPUP             = 1 << 0;
        /// Child window (clipped to parent).
        const CHILD             = 1 << 1;
        /// Has a minimize button.
        const MINIMIZE_BOX      = 1 << 2;
        /// Has a maximize button.
        const MAXIMIZE_BOX      = 1 << 3;
        /// Has a close button.
        const CLOSE_BOX         = 1 << 4;
        /// Has a title bar / caption.
        const CAPTION           = 1 << 5;
        /// Has a thin border.
        const BORDER            = 1 << 6;
        /// Has a thick (resizable) frame.
        const THICK_FRAME       = 1 << 7;
        /// Has a vertical scroll bar.
        const VSCROLL           = 1 << 8;
        /// Has a horizontal scroll bar.
        const HSCROLL           = 1 << 9;
        /// Has a system menu (window menu).
        const SYS_MENU          = 1 << 10;
        /// Clips child windows during painting.
        const CLIP_CHILDREN     = 1 << 11;
        /// Clips sibling windows during painting.
        const CLIP_SIBLINGS     = 1 << 12;

        /// Convenience: standard overlapped window.
        const OVERLAPPED_WINDOW = Self::CAPTION.bits()
                                | Self::SYS_MENU.bits()
                                | Self::THICK_FRAME.bits()
                                | Self::MINIMIZE_BOX.bits()
                                | Self::MAXIMIZE_BOX.bits()
                                | Self::CLOSE_BOX.bits()
                                | Self::BORDER.bits();
    }
}

bitflags! {
    /// Extended window style flags (analogous to WS_EX_*).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowExStyle: u32 {
        /// Always-on-top.
        const TOPMOST                   = 1 << 0;
        /// Transparent to mouse input.
        const TRANSPARENT               = 1 << 1;
        /// Tool window (small title bar, not in taskbar).
        const TOOL_WINDOW               = 1 << 2;
        /// Application window (forces taskbar presence).
        const APP_WINDOW                = 1 << 3;
        /// Layered / composited window.
        const LAYERED                   = 1 << 4;
        /// Does not activate on click.
        const NO_ACTIVATE               = 1 << 5;
        /// Double-buffered composited rendering.
        const COMPOSITED                = 1 << 6;
        /// Right-to-left layout (mirrored).
        const LAYOUT_RTL                = 1 << 7;
        /// No redirection bitmap (direct to screen).
        const NO_REDIRECTION_BITMAP     = 1 << 8;
    }
}
