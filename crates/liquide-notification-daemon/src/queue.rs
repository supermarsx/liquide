//! Priority notification queue with rate limiting.
//!
//! [`NotificationQueue`] orders pending notifications by urgency (Critical first,
//! then Normal, then Low) and enforces per-app rate limits.

use crate::rate_limiter::RateLimiter;
use crate::spec::{Notification, Urgency};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

/// Global monotonic ID counter for notification assignment.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Resets the global ID counter (for testing only).
#[cfg(test)]
pub(crate) fn reset_id_counter() {
    NEXT_ID.store(1, Ordering::Relaxed);
}

/// Allocates the next notification ID.
fn alloc_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Priority queue for pending notifications.
///
/// Internally uses three FIFO queues (one per urgency level). Dequeue always
/// drains Critical first, then Normal, then Low. Rate limiting is applied at
/// enqueue time.
pub struct NotificationQueue {
    /// Critical-urgency queue (drained first).
    critical: VecDeque<Notification>,
    /// Normal-urgency queue.
    normal: VecDeque<Notification>,
    /// Low-urgency queue (drained last).
    low: VecDeque<Notification>,
    /// Per-app rate limiter.
    rate_limiter: RateLimiter,
}

impl NotificationQueue {
    /// Creates a new queue with the default rate limit (5/sec/app).
    pub fn new() -> Self {
        Self {
            critical: VecDeque::new(),
            normal: VecDeque::new(),
            low: VecDeque::new(),
            rate_limiter: RateLimiter::default(),
        }
    }

    /// Creates a new queue with a custom rate limit.
    pub fn with_rate_limit(max_per_second: u32) -> Self {
        Self {
            critical: VecDeque::new(),
            normal: VecDeque::new(),
            low: VecDeque::new(),
            rate_limiter: RateLimiter::new(max_per_second),
        }
    }

    /// Enqueues a notification into the priority queue.
    ///
    /// Assigns a unique ID, checks rate limits, and inserts into the
    /// appropriate urgency bucket. Returns the assigned ID, or `None` if
    /// the notification was rate-limited.
    pub fn enqueue(&mut self, notification: Notification) -> Option<u32> {
        self.enqueue_at(notification, Self::now_ms())
    }

    /// Enqueues a notification with an explicit timestamp (for testing).
    pub fn enqueue_at(&mut self, mut notification: Notification, now_ms: u64) -> Option<u32> {
        // Rate-limit check (skip for Critical urgency and for notifications
        // that replace an existing one — the replacement is conceptually
        // an update to an already-visible notification and shouldn't count
        // against the app's per-second budget).
        let is_replacement = notification.replaces_id != 0;
        if !is_replacement
            && notification.urgency() != Urgency::Critical
            && !self.rate_limiter.check(&notification.app_name, now_ms)
        {
            return None;
        }

        // If this notification replaces an existing one, remove the old one.
        let replacement_id = if is_replacement {
            self.remove(notification.replaces_id)
                .map(|_| notification.replaces_id)
        } else {
            None
        };

        let id = replacement_id.unwrap_or_else(alloc_id);
        notification.id = id;

        let queue = match notification.urgency() {
            Urgency::Critical => &mut self.critical,
            Urgency::Normal => &mut self.normal,
            Urgency::Low => &mut self.low,
        };
        queue.push_back(notification);

        Some(id)
    }

    /// Dequeues the highest-priority notification.
    pub fn dequeue(&mut self) -> Option<Notification> {
        if let Some(n) = self.critical.pop_front() {
            return Some(n);
        }
        if let Some(n) = self.normal.pop_front() {
            return Some(n);
        }
        self.low.pop_front()
    }

    /// Peeks at the highest-priority notification without removing it.
    pub fn peek(&self) -> Option<&Notification> {
        if let Some(n) = self.critical.front() {
            return Some(n);
        }
        if let Some(n) = self.normal.front() {
            return Some(n);
        }
        self.low.front()
    }

    /// Removes a notification by ID from any queue. Returns it if found.
    pub fn remove(&mut self, id: u32) -> Option<Notification> {
        if let Some(pos) = self.critical.iter().position(|n| n.id == id) {
            return self.critical.remove(pos);
        }
        if let Some(pos) = self.normal.iter().position(|n| n.id == id) {
            return self.normal.remove(pos);
        }
        if let Some(pos) = self.low.iter().position(|n| n.id == id) {
            return self.low.remove(pos);
        }
        None
    }

    /// Returns the total number of pending notifications.
    pub fn pending_count(&self) -> usize {
        self.critical.len() + self.normal.len() + self.low.len()
    }

    /// Returns references to all notifications at the given urgency level.
    pub fn by_urgency(&self, urgency: Urgency) -> Vec<&Notification> {
        let queue = match urgency {
            Urgency::Critical => &self.critical,
            Urgency::Normal => &self.normal,
            Urgency::Low => &self.low,
        };
        queue.iter().collect()
    }

    /// Returns a mutable reference to the rate limiter.
    pub fn rate_limiter_mut(&mut self) -> &mut RateLimiter {
        &mut self.rate_limiter
    }

    /// Returns a reference to the rate limiter.
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// Platform-agnostic current time in ms (monotonic where available).
    fn now_ms() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for NotificationQueue {
    fn default() -> Self {
        Self::new()
    }
}
