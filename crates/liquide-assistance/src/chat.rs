//! Chat channel for assistance sessions.

use serde::{Deserialize, Serialize};

/// A chat message within an assistance session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Sender identifier.
    pub sender: String,
    /// Message text.
    pub text: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
    /// Sequence number within the channel.
    pub sequence: u64,
}

/// A chat channel for a shadow session.
pub struct ChatChannel {
    messages: Vec<ChatMessage>,
    next_sequence: u64,
    shadow_session_id: String,
}

impl ChatChannel {
    /// Create a new chat channel for the given session.
    #[must_use]
    pub fn new(session_id: String) -> Self {
        Self {
            messages: Vec::new(),
            next_sequence: 0,
            shadow_session_id: session_id,
        }
    }

    /// Send a message on the channel.
    pub fn send(&mut self, sender: String, text: String, timestamp: u64) -> ChatMessage {
        let msg = ChatMessage {
            sender,
            text,
            timestamp,
            sequence: self.next_sequence,
        };
        self.next_sequence += 1;
        self.messages.push(msg.clone());
        msg
    }

    /// All messages in the channel.
    #[must_use]
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Messages with sequence number greater than or equal to the given value.
    #[must_use]
    pub fn messages_since(&self, sequence: u64) -> &[ChatMessage] {
        // Messages are ordered by sequence, find the first matching index.
        let start = self
            .messages
            .iter()
            .position(|m| m.sequence >= sequence)
            .unwrap_or(self.messages.len());
        &self.messages[start..]
    }

    /// Total number of messages.
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// The session this channel belongs to.
    #[must_use]
    pub fn shadow_session_id(&self) -> &str {
        &self.shadow_session_id
    }
}
