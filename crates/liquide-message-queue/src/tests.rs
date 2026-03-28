//! Tests for the message queue subsystem.

use crate::filter::MessageFilter;
use crate::message::{MessageType, QueueMessage, WINDOW_BROADCAST};
use crate::pump::MessagePump;
use crate::queue::{Rect, ThreadQueue};
use crate::sent::SentMessage;
use crate::timer::TimerManager;
use crate::wake_bits::WakeBits;

// ── QueueMessage construction ───────────────────────────────────────────

#[test]
fn message_new_defaults() {
    let msg = QueueMessage::new(42, MessageType::Paint);
    assert_eq!(msg.target, 42);
    assert_eq!(msg.msg, MessageType::Paint);
    assert_eq!(msg.wparam, 0);
    assert_eq!(msg.lparam, 0);
    assert_eq!(msg.time, 0);
    assert_eq!(msg.pt, (0, 0));
    assert_eq!(msg.extra_info, 0);
}

#[test]
fn message_builder_chain() {
    let msg = QueueMessage::new(1, MessageType::KeyDown)
        .with_wparam(65)
        .with_lparam(-1)
        .with_time(12345)
        .with_pt(100, 200)
        .with_extra_info(99);
    assert_eq!(msg.wparam, 65);
    assert_eq!(msg.lparam, -1);
    assert_eq!(msg.time, 12345);
    assert_eq!(msg.pt, (100, 200));
    assert_eq!(msg.extra_info, 99);
}

// ── MessageType ─────────────────────────────────────────────────────────

#[test]
fn message_type_is_mouse() {
    assert!(MessageType::MouseMove.is_mouse());
    assert!(MessageType::MouseDown.is_mouse());
    assert!(MessageType::MouseUp.is_mouse());
    assert!(MessageType::MouseWheel.is_mouse());
    assert!(MessageType::MouseEnter.is_mouse());
    assert!(MessageType::MouseLeave.is_mouse());
    assert!(!MessageType::KeyDown.is_mouse());
    assert!(!MessageType::Paint.is_mouse());
}

#[test]
fn message_type_is_key() {
    assert!(MessageType::KeyDown.is_key());
    assert!(MessageType::KeyUp.is_key());
    assert!(MessageType::KeyChar.is_key());
    assert!(!MessageType::MouseMove.is_key());
}

#[test]
fn message_type_is_input() {
    assert!(MessageType::MouseDown.is_input());
    assert!(MessageType::KeyDown.is_input());
    assert!(!MessageType::Paint.is_input());
    assert!(!MessageType::Quit.is_input());
}

#[test]
fn message_type_discriminant_ordering() {
    // Mouse types cluster together
    assert!(MessageType::MouseMove.discriminant() < MessageType::KeyDown.discriminant());
    // Paint is before input
    assert!(MessageType::Paint.discriminant() < MessageType::MouseMove.discriminant());
    // Timer has its own discriminant
    assert_eq!(MessageType::Timer(1).discriminant(), MessageType::Timer(99).discriminant());
}

// ── WakeBits ────────────────────────────────────────────────────────────

#[test]
fn wake_bits_none_is_empty() {
    assert!(WakeBits::NONE.is_empty());
    assert!(!WakeBits::QS_PAINT.is_empty());
}

#[test]
fn wake_bits_insert_remove() {
    let mut bits = WakeBits::NONE;
    bits.insert(WakeBits::QS_KEY);
    assert!(bits.contains(WakeBits::QS_KEY));
    assert!(!bits.contains(WakeBits::QS_MOUSE));
    bits.insert(WakeBits::QS_MOUSE);
    assert!(bits.intersects(WakeBits::QS_INPUT));
    bits.remove(WakeBits::QS_KEY);
    assert!(!bits.contains(WakeBits::QS_KEY));
    assert!(bits.contains(WakeBits::QS_MOUSE));
}

#[test]
fn wake_bits_composite_masks() {
    assert!(WakeBits::QS_INPUT.contains(WakeBits::QS_KEY));
    assert!(WakeBits::QS_INPUT.contains(WakeBits::QS_MOUSE));
    assert!(WakeBits::QS_INPUT.contains(WakeBits::QS_MOUSEMOVE));
    assert!(!WakeBits::QS_INPUT.contains(WakeBits::QS_PAINT));

    assert!(WakeBits::QS_ALLINPUT.contains(WakeBits::QS_INPUT));
    assert!(WakeBits::QS_ALLINPUT.contains(WakeBits::QS_PAINT));
    assert!(WakeBits::QS_ALLINPUT.contains(WakeBits::QS_TIMER));
    assert!(WakeBits::QS_ALLINPUT.contains(WakeBits::QS_SENDMESSAGE));
    assert!(WakeBits::QS_ALLINPUT.contains(WakeBits::QS_POSTMESSAGE));
    assert!(WakeBits::QS_ALLINPUT.contains(WakeBits::QS_HOTKEY));
}

#[test]
fn wake_bits_intersects() {
    let bits = WakeBits::QS_KEY | WakeBits::QS_PAINT;
    assert!(bits.intersects(WakeBits::QS_KEY));
    assert!(bits.intersects(WakeBits::QS_PAINT));
    assert!(!bits.intersects(WakeBits::QS_TIMER));
    assert!(bits.intersects(WakeBits::QS_INPUT));
}

#[test]
fn wake_bits_bitwise_ops() {
    let a = WakeBits::QS_KEY;
    let b = WakeBits::QS_MOUSE;
    let c = a | b;
    assert!(c.contains(WakeBits::QS_KEY));
    assert!(c.contains(WakeBits::QS_MOUSE));
    let d = c & WakeBits::QS_KEY;
    assert!(d.contains(WakeBits::QS_KEY));
    assert!(!d.contains(WakeBits::QS_MOUSE));
}

#[test]
fn wake_bits_not() {
    let bits = !WakeBits::NONE;
    assert!(bits.contains(WakeBits::QS_ALLINPUT));
}

// ── ThreadQueue: post and peek ──────────────────────────────────────────

#[test]
fn queue_post_and_peek() {
    let mut q = ThreadQueue::new(1);
    assert!(!q.has_messages());

    q.post_message(QueueMessage::new(10, MessageType::WindowCreated));
    assert!(q.has_messages());
    assert_eq!(q.posted_count(), 1);

    let msg = q.peek_message(None, false).unwrap();
    assert_eq!(msg.msg, MessageType::WindowCreated);
    assert_eq!(msg.target, 10);
    // Still in queue (peek, no remove)
    assert_eq!(q.posted_count(), 1);

    // Now remove
    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::WindowCreated);
    assert_eq!(q.posted_count(), 0);
}

#[test]
fn queue_fifo_order() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::Show));
    q.post_message(QueueMessage::new(1, MessageType::FocusGained));
    q.post_message(QueueMessage::new(1, MessageType::Paint));

    // Note: Paint posted explicitly goes into FIFO, not synthesized.
    let m1 = q.peek_message(None, true).unwrap();
    assert_eq!(m1.msg, MessageType::Show);
    let m2 = q.peek_message(None, true).unwrap();
    assert_eq!(m2.msg, MessageType::FocusGained);
    let m3 = q.peek_message(None, true).unwrap();
    assert_eq!(m3.msg, MessageType::Paint);
}

#[test]
fn queue_wake_bits_updated_on_post() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::KeyDown));
    assert!(q.wake_bits().contains(WakeBits::QS_KEY));

    q.post_message(QueueMessage::new(1, MessageType::MouseDown));
    assert!(q.wake_bits().contains(WakeBits::QS_MOUSE));

    // Remove key message
    let _ = q.peek_message(Some(MessageFilter::single(MessageType::KeyDown, true)), true);
    // Key bit should be cleared (no more key messages)
    assert!(!q.wake_bits().contains(WakeBits::QS_KEY));
    // Mouse bit still set
    assert!(q.wake_bits().contains(WakeBits::QS_MOUSE));
}

// ── Mouse-move coalescing ───────────────────────────────────────────────

#[test]
fn mouse_move_coalescing() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::MouseMove).with_pt(10, 10));
    q.post_message(QueueMessage::new(1, MessageType::MouseMove).with_pt(20, 20));
    q.post_message(QueueMessage::new(1, MessageType::MouseMove).with_pt(30, 30));

    // Only the last mouse move should be returned
    assert_eq!(q.posted_count(), 0); // mouse moves go to coalesced slot, not FIFO
    assert!(q.wake_bits().contains(WakeBits::QS_MOUSEMOVE));

    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::MouseMove);
    assert_eq!(msg.pt, (30, 30));

    // No more mouse moves
    assert!(!q.wake_bits().contains(WakeBits::QS_MOUSEMOVE));
}

#[test]
fn mouse_move_does_not_block_other_messages() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::KeyDown));
    q.post_message(QueueMessage::new(1, MessageType::MouseMove).with_pt(50, 50));

    // Posted messages have higher priority than coalesced mouse move
    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::KeyDown);

    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::MouseMove);
}

// ── Paint coalescing ────────────────────────────────────────────────────

#[test]
fn paint_invalidation_and_synthesis() {
    let mut q = ThreadQueue::new(1);
    q.invalidate_window(10, Some(Rect::new(0.0, 0.0, 100.0, 100.0)));
    assert!(q.wake_bits().contains(WakeBits::QS_PAINT));

    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::Paint);
    assert_eq!(msg.target, 10);

    // After retrieval, paint bit should be cleared
    assert!(!q.wake_bits().contains(WakeBits::QS_PAINT));
}

#[test]
fn paint_multiple_invalidations_coalesce() {
    let mut q = ThreadQueue::new(1);
    q.invalidate_window(10, Some(Rect::new(0.0, 0.0, 50.0, 50.0)));
    q.invalidate_window(10, Some(Rect::new(30.0, 30.0, 50.0, 50.0)));

    // Should produce a single paint with the union rect
    let region = q.invalid_region(10).unwrap();
    assert_eq!(region.x, 0.0);
    assert_eq!(region.y, 0.0);
    assert_eq!(region.width, 80.0);
    assert_eq!(region.height, 80.0);

    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::Paint);
    assert!(q.peek_message(Some(MessageFilter::single(MessageType::Paint, false)), false).is_none());
}

#[test]
fn paint_validate_clears_dirty() {
    let mut q = ThreadQueue::new(1);
    q.invalidate_window(10, None);
    assert!(q.wake_bits().contains(WakeBits::QS_PAINT));

    q.validate_window(10, None);
    assert!(!q.wake_bits().contains(WakeBits::QS_PAINT));
    assert!(q.dirty_windows().is_empty());
}

#[test]
fn paint_has_lowest_priority_except_timer() {
    let mut q = ThreadQueue::new(1);
    q.invalidate_window(10, None);
    q.post_message(QueueMessage::new(10, MessageType::Show));

    // Posted message comes before synthetic paint
    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::Show);

    // Now paint
    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::Paint);
}

// ── Capture ─────────────────────────────────────────────────────────────

#[test]
fn capture_set_and_release() {
    let mut q = ThreadQueue::new(1);
    assert_eq!(q.capture_window(), None);

    q.set_capture(42);
    assert_eq!(q.capture_window(), Some(42));

    let prev = q.release_capture();
    assert_eq!(prev, Some(42));
    assert_eq!(q.capture_window(), None);
}

// ── Focus and activation ────────────────────────────────────────────────

#[test]
fn focus_window_posts_messages() {
    let mut q = ThreadQueue::new(1);
    q.set_focus_window(10);
    assert_eq!(q.focus_window(), Some(10));

    // Should have posted FocusGained
    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::FocusGained);
    assert_eq!(msg.target, 10);

    // Change focus
    q.set_focus_window(20);
    let lost = q.peek_message(None, true).unwrap();
    assert_eq!(lost.msg, MessageType::FocusLost);
    assert_eq!(lost.target, 10);

    let gained = q.peek_message(None, true).unwrap();
    assert_eq!(gained.msg, MessageType::FocusGained);
    assert_eq!(gained.target, 20);
}

#[test]
fn active_window_posts_messages() {
    let mut q = ThreadQueue::new(1);
    q.set_active_window(10);
    assert_eq!(q.active_window(), Some(10));

    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::Activate);

    q.set_active_window(20);
    let deact = q.peek_message(None, true).unwrap();
    assert_eq!(deact.msg, MessageType::Deactivate);
    assert_eq!(deact.target, 10);

    let act = q.peek_message(None, true).unwrap();
    assert_eq!(act.msg, MessageType::Activate);
    assert_eq!(act.target, 20);
}

#[test]
fn clear_focus_posts_focus_lost() {
    let mut q = ThreadQueue::new(1);
    q.set_focus_window(10);
    let _ = q.peek_message(None, true); // drain FocusGained

    q.clear_focus_window();
    assert_eq!(q.focus_window(), None);

    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::FocusLost);
    assert_eq!(msg.target, 10);
}

#[test]
fn set_focus_same_window_noop() {
    let mut q = ThreadQueue::new(1);
    q.set_focus_window(10);
    let _ = q.peek_message(None, true); // drain FocusGained

    q.set_focus_window(10); // same window, should be noop
    assert!(q.peek_message(None, false).is_none());
}

// ── MessageFilter ───────────────────────────────────────────────────────

#[test]
fn filter_matches_all() {
    let f = MessageFilter::all();
    let msg = QueueMessage::new(1, MessageType::KeyDown);
    assert!(f.matches(&msg));
}

#[test]
fn filter_window() {
    let f = MessageFilter::for_window(10, true);
    assert!(f.matches(&QueueMessage::new(10, MessageType::Paint)));
    assert!(!f.matches(&QueueMessage::new(20, MessageType::Paint)));
}

#[test]
fn filter_range() {
    // Only mouse messages (discriminants 10-15)
    let f = MessageFilter::range(MessageType::MouseMove, MessageType::MouseLeave, true);
    assert!(f.matches(&QueueMessage::new(1, MessageType::MouseDown)));
    assert!(f.matches(&QueueMessage::new(1, MessageType::MouseMove)));
    assert!(!f.matches(&QueueMessage::new(1, MessageType::KeyDown)));
    assert!(!f.matches(&QueueMessage::new(1, MessageType::Paint)));
}

#[test]
fn filter_single() {
    let f = MessageFilter::single(MessageType::Close, true);
    assert!(f.matches(&QueueMessage::new(1, MessageType::Close)));
    assert!(!f.matches(&QueueMessage::new(1, MessageType::Quit)));
}

#[test]
fn peek_with_filter_skips_non_matching() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::Show));
    q.post_message(QueueMessage::new(1, MessageType::KeyDown));
    q.post_message(QueueMessage::new(1, MessageType::Hide));

    // Filter for key messages only
    let f = MessageFilter::single(MessageType::KeyDown, true);
    let msg = q.peek_message(Some(f), true).unwrap();
    assert_eq!(msg.msg, MessageType::KeyDown);
    // Show and Hide should still be in the queue
    assert_eq!(q.posted_count(), 2);
}

// ── Timers ──────────────────────────────────────────────────────────────

#[test]
fn timer_basic_fire() {
    let mut tm = TimerManager::new();
    tm.set_timer(10, 1, 100, 1_000_000); // 100ms interval, start at 1s

    // Not yet expired
    let msgs = tm.check_timers(1_050_000);
    assert!(msgs.is_empty());

    // Expired
    let msgs = tm.check_timers(1_100_000);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].target, 10);
    assert!(matches!(msgs[0].msg, MessageType::Timer(1)));
}

#[test]
fn timer_reschedules_after_fire() {
    let mut tm = TimerManager::new();
    tm.set_timer(10, 1, 100, 1_000_000);

    // Fire once
    let _ = tm.check_timers(1_100_000);

    // Not yet next interval
    let msgs = tm.check_timers(1_150_000);
    assert!(msgs.is_empty());

    // Next fire at 1_200_000
    let msgs = tm.check_timers(1_200_000);
    assert_eq!(msgs.len(), 1);
}

#[test]
fn timer_skips_missed_intervals() {
    let mut tm = TimerManager::new();
    tm.set_timer(10, 1, 100, 1_000_000); // fires at 1.1s

    // Jump far ahead — should only fire once, not storm
    let msgs = tm.check_timers(2_000_000);
    assert_eq!(msgs.len(), 1);

    // Next fire should be in the future relative to 2s
    let msgs = tm.check_timers(2_050_000);
    assert!(msgs.is_empty());
}

#[test]
fn timer_replace_existing() {
    let mut tm = TimerManager::new();
    tm.set_timer(10, 1, 100, 1_000_000);
    tm.set_timer(10, 1, 200, 1_000_000); // replace with 200ms interval

    assert_eq!(tm.count(), 1);

    // Old timer would fire at 1.1s, new at 1.2s
    let msgs = tm.check_timers(1_100_000);
    assert!(msgs.is_empty());

    let msgs = tm.check_timers(1_200_000);
    assert_eq!(msgs.len(), 1);
}

#[test]
fn timer_kill() {
    let mut tm = TimerManager::new();
    tm.set_timer(10, 1, 100, 1_000_000);
    assert_eq!(tm.count(), 1);

    assert!(tm.kill_timer(10, 1));
    assert_eq!(tm.count(), 0);
    assert!(!tm.kill_timer(10, 1)); // already dead
}

#[test]
fn timer_kill_all_for_window() {
    let mut tm = TimerManager::new();
    tm.set_timer(10, 1, 100, 0);
    tm.set_timer(10, 2, 200, 0);
    tm.set_timer(20, 1, 100, 0);
    assert_eq!(tm.count(), 3);

    tm.kill_all_for_window(10);
    assert_eq!(tm.count(), 1);
}

#[test]
fn timer_nearest_deadline() {
    let mut tm = TimerManager::new();
    assert_eq!(tm.nearest_deadline(), None);

    tm.set_timer(10, 1, 100, 1_000_000); // fires at 1.1s
    tm.set_timer(10, 2, 50, 1_000_000); // fires at 1.05s

    assert_eq!(tm.nearest_deadline(), Some(1_050_000));
}

#[test]
fn timer_with_callback() {
    let mut tm = TimerManager::new();
    tm.set_timer_with_callback(10, 1, 100, 1_000_000, 0xDEAD);

    let msgs = tm.check_timers(1_100_000);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].lparam, 0xDEAD_i64);
}

#[test]
fn queue_set_timer_and_check() {
    let mut q = ThreadQueue::new(1);
    let now = 1_000_000u64;
    q.set_timer_at(10, 1, 50, now);

    let msgs = q.check_timers(now + 50_000);
    assert_eq!(msgs.len(), 1);
    assert!(matches!(msgs[0].msg, MessageType::Timer(1)));
}

// ── SentMessage ─────────────────────────────────────────────────────────

#[test]
fn sent_message_reply() {
    let sm = SentMessage::new(QueueMessage::new(1, MessageType::Close), 99);
    assert!(!sm.is_replied());
    assert!(sm.try_get_result().is_none());

    sm.reply(42);
    assert!(sm.is_replied());
    assert_eq!(sm.try_get_result(), Some(42));
    assert_eq!(sm.wait_for_reply(), 42);
}

#[test]
fn sent_message_process_in_queue() {
    let mut q = ThreadQueue::new(1);
    let sm = SentMessage::new(QueueMessage::new(10, MessageType::Close), 2);
    let sm_clone = sm.clone();

    q.push_sent_message(sm);
    assert!(q.wake_bits().contains(WakeBits::QS_SENDMESSAGE));
    assert_eq!(q.sent_count(), 1);

    q.process_sent_messages(&mut |msg| {
        assert_eq!(msg.msg, MessageType::Close);
        assert_eq!(msg.target, 10);
        7
    });

    assert_eq!(q.sent_count(), 0);
    assert!(!q.wake_bits().contains(WakeBits::QS_SENDMESSAGE));
    assert_eq!(sm_clone.wait_for_reply(), 7);
}

// ── Purge window ────────────────────────────────────────────────────────

#[test]
fn purge_window_removes_all_related() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(10, MessageType::Show));
    q.post_message(QueueMessage::new(10, MessageType::FocusGained));
    q.post_message(QueueMessage::new(20, MessageType::Show));
    q.post_message(QueueMessage::new(10, MessageType::MouseMove).with_pt(5, 5));
    q.invalidate_window(10, None);
    q.set_timer_at(10, 1, 100, 0);
    q.set_capture(10);
    q.set_focus_window(10);
    // drain focus gained msg
    let _ = q.peek_message(Some(MessageFilter::for_window(10, true).with_range(MessageType::FocusGained, MessageType::FocusGained)), true);
    q.set_active_window(10);
    // drain activate msg
    let _ = q.peek_message(Some(MessageFilter::for_window(10, true).with_range(MessageType::Activate, MessageType::Activate)), true);

    q.purge_window(10);

    assert_eq!(q.capture_window(), None);
    assert_eq!(q.focus_window(), None);
    assert_eq!(q.active_window(), None);
    assert!(q.dirty_windows().is_empty());

    // Only window 20's message should remain
    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.target, 20);
    assert_eq!(msg.msg, MessageType::Show);
}

// ── Clear and Quit ──────────────────────────────────────────────────────

#[test]
fn queue_clear() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::KeyDown));
    q.invalidate_window(1, None);
    q.clear();
    assert!(!q.has_messages());
    assert_eq!(q.posted_count(), 0);
}

#[test]
fn queue_post_quit() {
    let mut q = ThreadQueue::new(1);
    q.post_quit();
    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::Quit);
    assert_eq!(msg.target, WINDOW_BROADCAST);
}

// ── MessagePump ─────────────────────────────────────────────────────────

#[test]
fn pump_bounded_processes_messages() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::Show));
    q.post_message(QueueMessage::new(1, MessageType::KeyDown));

    let pump = MessagePump::new();
    let mut handled = Vec::new();
    let result = pump.run_bounded(&mut q, &mut |msg: &QueueMessage| -> i64 {
        handled.push(msg.msg);
        0
    }, 10);

    assert_eq!(result, None); // no Quit
    assert_eq!(handled, vec![MessageType::Show, MessageType::KeyDown]);
}

#[test]
fn pump_bounded_stops_on_quit() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::Show));
    q.post_message(QueueMessage::new(WINDOW_BROADCAST, MessageType::Quit).with_wparam(42));
    q.post_message(QueueMessage::new(1, MessageType::KeyDown));

    let pump = MessagePump::new();
    let mut count = 0;
    let result = pump.run_bounded(&mut q, &mut |_msg: &QueueMessage| -> i64 {
        count += 1;
        0
    }, 10);

    assert_eq!(result, Some(42));
    assert_eq!(count, 1); // only Show was dispatched before Quit
}

#[test]
fn pump_one_dispatches_single() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::WindowCreated));

    let pump = MessagePump::new();
    let result = pump.pump_one(&mut q, &mut |msg: &QueueMessage| -> i64 {
        assert_eq!(msg.msg, MessageType::WindowCreated);
        99
    });
    assert_eq!(result, Some(Ok(99)));

    // Nothing left
    let result = pump.pump_one(&mut q, &mut |_: &QueueMessage| 0);
    assert!(result.is_none());
}

#[test]
fn pump_one_returns_quit() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(WINDOW_BROADCAST, MessageType::Quit).with_wparam(7));

    let pump = MessagePump::new();
    let result = pump.pump_one(&mut q, &mut |_: &QueueMessage| 0);
    assert_eq!(result, Some(Err(7)));
}

#[test]
fn pump_processes_sent_before_posted() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::Show));
    let sm = SentMessage::new(QueueMessage::new(1, MessageType::Close), 2);
    let sm_clone = sm.clone();
    q.push_sent_message(sm);

    let pump = MessagePump::new();
    let mut order = Vec::new();
    // pump_one processes sent messages first, then dispatches the next posted
    let _ = pump.pump_one(&mut q, &mut |msg: &QueueMessage| -> i64 {
        order.push(msg.msg);
        55
    });

    // The handler is used for both sent and posted messages:
    // 1. process_sent_messages calls handler for Close (sent message)
    // 2. pump_one dispatches Show (posted message) via handler
    // So the handler sees Close first, then Show.
    assert_eq!(sm_clone.wait_for_reply(), 55);
    assert_eq!(order, vec![MessageType::Close, MessageType::Show]);
}

// ── Rect ────────────────────────────────────────────────────────────────

#[test]
fn rect_union() {
    let a = Rect::new(10.0, 10.0, 50.0, 50.0);
    let b = Rect::new(40.0, 40.0, 50.0, 50.0);
    let u = a.union(b);
    assert_eq!(u.x, 10.0);
    assert_eq!(u.y, 10.0);
    assert_eq!(u.width, 80.0);
    assert_eq!(u.height, 80.0);
}

#[test]
fn rect_is_empty() {
    assert!(Rect::new(0.0, 0.0, 0.0, 10.0).is_empty());
    assert!(Rect::new(0.0, 0.0, 10.0, 0.0).is_empty());
    assert!(Rect::new(0.0, 0.0, -1.0, 10.0).is_empty());
    assert!(!Rect::new(0.0, 0.0, 1.0, 1.0).is_empty());
}

// ── Multiple windows with separate invalidation ────────────────────────

#[test]
fn multiple_windows_paint_independently() {
    let mut q = ThreadQueue::new(1);
    q.invalidate_window(10, None);
    q.invalidate_window(20, None);

    let dirty = q.dirty_windows();
    assert!(dirty.contains(&10));
    assert!(dirty.contains(&20));

    q.validate_window(10, None);
    let dirty = q.dirty_windows();
    assert!(!dirty.contains(&10));
    assert!(dirty.contains(&20));
}

// ── Edge case: filter with window + range ───────────────────────────────

#[test]
fn filter_combined_window_and_range() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(10, MessageType::KeyDown));
    q.post_message(QueueMessage::new(20, MessageType::KeyDown));
    q.post_message(QueueMessage::new(10, MessageType::MouseDown));

    let f = MessageFilter::for_window(10, true)
        .with_range(MessageType::KeyDown, MessageType::KeyChar);
    let msg = q.peek_message(Some(f), true).unwrap();
    assert_eq!(msg.target, 10);
    assert_eq!(msg.msg, MessageType::KeyDown);
    assert_eq!(q.posted_count(), 2); // window 20's key + window 10's mouse remain
}

// ── Custom and Noop message types ───────────────────────────────────────

#[test]
fn custom_and_noop_messages() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(1, MessageType::Custom(999)));
    q.post_message(QueueMessage::new(1, MessageType::Noop));

    let m1 = q.peek_message(None, true).unwrap();
    assert_eq!(m1.msg, MessageType::Custom(999));

    let m2 = q.peek_message(None, true).unwrap();
    assert_eq!(m2.msg, MessageType::Noop);
}

#[test]
fn hotkey_message() {
    let mut q = ThreadQueue::new(1);
    q.post_message(QueueMessage::new(WINDOW_BROADCAST, MessageType::HotKey(42)));
    assert!(q.wake_bits().contains(WakeBits::QS_HOTKEY));

    let msg = q.peek_message(None, true).unwrap();
    assert_eq!(msg.msg, MessageType::HotKey(42));
}

// ── Thread queue debug formatting ───────────────────────────────────────

#[test]
fn queue_debug_format() {
    let q = ThreadQueue::new(42);
    let debug = format!("{:?}", q);
    assert!(debug.contains("ThreadQueue"));
    assert!(debug.contains("42"));
}
