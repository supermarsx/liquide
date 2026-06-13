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
        // A notification only counts as a replacement if its `replaces_id`
        // refers to a notification that is genuinely present in the queue.
        // Removing it up front establishes that fact: if `remove` finds and
        // returns the old notification, this is a real update to an
        // already-tracked one and we reuse its ID. A bogus or stale
        // `replaces_id` (0, or an id not in any bucket) yields `None` here and
        // is treated as a brand-new notification — otherwise a flood of fake
        // `replaces_id` values would bypass the per-app rate limiter.
        let replacement_id = if notification.replaces_id != 0 {
            self.remove(notification.replaces_id)
                .map(|_| notification.replaces_id)
        } else {
            None
        };
        let is_replacement = replacement_id.is_some();

        // Rate-limit check (skip for Critical urgency and for genuine
        // replacements — a replacement is conceptually an update to an
        // already-visible notification and shouldn't count against the app's
        // per-second budget).
        if !is_replacement
            && notification.urgency() != Urgency::Critical
            && !self.rate_limiter.check(&notification.app_name, now_ms)
        {
            return None;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Notification;

    fn app_notification(app: &str) -> Notification {
        Notification::new("summary").with_app_name(app)
    }

    /// A burst of notifications carrying bogus (non-existent) `replaces_id`
    /// values must be subject to the rate limiter exactly like brand-new
    /// notifications — it must NOT bypass throttling.
    #[test]
    fn bogus_replaces_id_does_not_bypass_rate_limit() {
        reset_id_counter();
        let mut queue = NotificationQueue::with_rate_limit(3);

        // 5 notifications, each claiming to replace a notification that was
        // never enqueued (ids 9000.. are not present in any bucket).
        let mut accepted = 0;
        for i in 0..5 {
            let n = app_notification("flooder").with_replaces_id(9000 + i);
            if queue.enqueue_at(n, 0).is_some() {
                accepted += 1;
            }
        }

        // Only the rate-limit budget (3/sec) should have been accepted; the
        // fake replaces_id must not have waived the limit.
        assert_eq!(accepted, 3, "bogus replaces_id bypassed the rate limiter");
        assert_eq!(queue.pending_count(), 3);
    }

    /// A genuine replacement of a notification that is actually present in the
    /// queue still bypasses the rate limit, as intended.
    #[test]
    fn genuine_replacement_bypasses_rate_limit() {
        reset_id_counter();
        let mut queue = NotificationQueue::with_rate_limit(1);

        // First notification consumes the entire 1/sec budget.
        let first = app_notification("app");
        let first_id = queue
            .enqueue_at(first, 0)
            .expect("first should be accepted");

        // A new notification from the same app at the same instant is
        // rate-limited.
        assert!(
            queue.enqueue_at(app_notification("app"), 0).is_none(),
            "second new notification should be rate-limited"
        );

        // But a genuine replacement of the existing notification is waived and
        // reuses the original id.
        let replacement = app_notification("app").with_replaces_id(first_id);
        let replaced_id = queue
            .enqueue_at(replacement, 0)
            .expect("genuine replacement should be accepted despite the rate limit");
        assert_eq!(replaced_id, first_id, "replacement should reuse the old id");

        // The replacement updated the existing notification rather than adding
        // a new one, so the queue still holds a single notification.
        assert_eq!(queue.pending_count(), 1);
    }
}
