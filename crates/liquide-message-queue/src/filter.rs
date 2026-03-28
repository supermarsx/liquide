//! Message filtering for `PeekMessage` / `GetMessage`.
//!
//! A [`MessageFilter`] restricts which messages are returned by the peek/get
//! operations, analogous to Win32's `PeekMessage(... wMsgFilterMin,
//! wMsgFilterMax, wRemoveMsg)` parameters.

use crate::message::{MessageType, QueueMessage, WindowId};

/// Filter for [`ThreadQueue::peek_message`](crate::ThreadQueue::peek_message).
#[derive(Debug, Clone)]
pub struct MessageFilter {
    /// Only return messages whose discriminant falls within this inclusive
    /// range.  `None` means accept all message types.
    pub msg_range: Option<(MessageType, MessageType)>,

    /// Only return messages targeted at this window.  `None` means any window.
    pub window: Option<WindowId>,

    /// Whether to remove the message from the queue when it matches
    /// (`PM_REMOVE`) or leave it (`PM_NOREMOVE`).
    pub remove: bool,
}

impl MessageFilter {
    /// Accept all messages, remove on match.
    #[must_use]
    pub fn all() -> Self {
        Self {
            msg_range: None,
            window: None,
            remove: true,
        }
    }

    /// Accept all messages, do not remove.
    #[must_use]
    pub fn peek_all() -> Self {
        Self {
            msg_range: None,
            window: None,
            remove: false,
        }
    }

    /// Accept messages of a single type only.
    #[must_use]
    pub fn single(msg: MessageType, remove: bool) -> Self {
        Self {
            msg_range: Some((msg, msg)),
            window: None,
            remove,
        }
    }

    /// Accept messages in a discriminant range (inclusive).
    #[must_use]
    pub fn range(min: MessageType, max: MessageType, remove: bool) -> Self {
        Self {
            msg_range: Some((min, max)),
            window: None,
            remove,
        }
    }

    /// Accept messages for a specific window only.
    #[must_use]
    pub fn for_window(window_id: WindowId, remove: bool) -> Self {
        Self {
            msg_range: None,
            window: Some(window_id),
            remove,
        }
    }

    /// Builder: set message range.
    #[must_use]
    pub fn with_range(mut self, min: MessageType, max: MessageType) -> Self {
        self.msg_range = Some((min, max));
        self
    }

    /// Builder: set window filter.
    #[must_use]
    pub fn with_window(mut self, window_id: WindowId) -> Self {
        self.window = Some(window_id);
        self
    }

    /// Builder: set remove flag.
    #[must_use]
    pub fn with_remove(mut self, remove: bool) -> Self {
        self.remove = remove;
        self
    }

    /// Test whether a message matches this filter.
    #[must_use]
    pub fn matches(&self, msg: &QueueMessage) -> bool {
        // Window filter
        if let Some(wid) = self.window {
            if msg.target != wid {
                return false;
            }
        }
        // Message type range filter
        if let Some((ref min, ref max)) = self.msg_range {
            let d = msg.msg.discriminant();
            let lo = min.discriminant();
            let hi = max.discriminant();
            if d < lo || d > hi {
                return false;
            }
        }
        true
    }
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self::all()
    }
}
