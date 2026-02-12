use liquide_protocol::state::*;

// ── ChannelState tests ─────────────────────────────────────────

#[test]
fn channel_default_is_closed() {
    assert_eq!(ChannelState::default(), ChannelState::Closed);
}

#[test]
fn channel_closed_to_opening() {
    let s = ChannelState::Closed.transition(ChannelEvent::Open).unwrap();
    assert_eq!(s, ChannelState::Opening);
}

#[test]
fn channel_opening_to_active() {
    let s = ChannelState::Opening.transition(ChannelEvent::Ack).unwrap();
    assert_eq!(s, ChannelState::Active);
}

#[test]
fn channel_opening_to_rejected() {
    let s = ChannelState::Opening
        .transition(ChannelEvent::Reject)
        .unwrap();
    assert_eq!(s, ChannelState::Rejected);
}

#[test]
fn channel_active_to_suspended() {
    let s = ChannelState::Active
        .transition(ChannelEvent::Suspend)
        .unwrap();
    assert_eq!(s, ChannelState::Suspended);
}

#[test]
fn channel_active_to_closed() {
    let s = ChannelState::Active
        .transition(ChannelEvent::Close)
        .unwrap();
    assert_eq!(s, ChannelState::Closed);
}

#[test]
fn channel_active_reset_to_opening() {
    let s = ChannelState::Active
        .transition(ChannelEvent::Reset)
        .unwrap();
    assert_eq!(s, ChannelState::Opening);
}

#[test]
fn channel_suspended_resume_to_active() {
    let s = ChannelState::Suspended
        .transition(ChannelEvent::Resume)
        .unwrap();
    assert_eq!(s, ChannelState::Active);
}

#[test]
fn channel_suspended_close_to_closed() {
    let s = ChannelState::Suspended
        .transition(ChannelEvent::Close)
        .unwrap();
    assert_eq!(s, ChannelState::Closed);
}

#[test]
fn channel_rejected_reopen() {
    let s = ChannelState::Rejected
        .transition(ChannelEvent::Open)
        .unwrap();
    assert_eq!(s, ChannelState::Opening);
}

#[test]
fn channel_is_active_only_when_active() {
    assert!(!ChannelState::Closed.is_active());
    assert!(!ChannelState::Opening.is_active());
    assert!(ChannelState::Active.is_active());
    assert!(!ChannelState::Suspended.is_active());
    assert!(!ChannelState::Rejected.is_active());
}

#[test]
fn channel_full_lifecycle() {
    let s = ChannelState::default();
    let s = s.transition(ChannelEvent::Open).unwrap();
    assert_eq!(s, ChannelState::Opening);
    let s = s.transition(ChannelEvent::Ack).unwrap();
    assert_eq!(s, ChannelState::Active);
    let s = s.transition(ChannelEvent::Suspend).unwrap();
    assert_eq!(s, ChannelState::Suspended);
    let s = s.transition(ChannelEvent::Resume).unwrap();
    assert_eq!(s, ChannelState::Active);
    let s = s.transition(ChannelEvent::Close).unwrap();
    assert_eq!(s, ChannelState::Closed);
}

#[test]
fn channel_reject_then_reopen_lifecycle() {
    let s = ChannelState::Closed;
    let s = s.transition(ChannelEvent::Open).unwrap();
    let s = s.transition(ChannelEvent::Reject).unwrap();
    assert_eq!(s, ChannelState::Rejected);
    let s = s.transition(ChannelEvent::Open).unwrap();
    assert_eq!(s, ChannelState::Opening);
    let s = s.transition(ChannelEvent::Ack).unwrap();
    assert_eq!(s, ChannelState::Active);
}

#[test]
fn channel_reset_lifecycle() {
    let s = ChannelState::Active;
    let s = s.transition(ChannelEvent::Reset).unwrap();
    assert_eq!(s, ChannelState::Opening);
    let s = s.transition(ChannelEvent::Ack).unwrap();
    assert_eq!(s, ChannelState::Active);
}

// ── Invalid ChannelState transitions ───────────────────────────

#[test]
fn channel_invalid_closed_ack() {
    let err = ChannelState::Closed
        .transition(ChannelEvent::Ack)
        .unwrap_err();
    assert_eq!(err.from, ChannelState::Closed);
    assert_eq!(err.event, ChannelEvent::Ack);
}

#[test]
fn channel_invalid_closed_close() {
    assert!(
        ChannelState::Closed
            .transition(ChannelEvent::Close)
            .is_err()
    );
}

#[test]
fn channel_invalid_closed_suspend() {
    assert!(
        ChannelState::Closed
            .transition(ChannelEvent::Suspend)
            .is_err()
    );
}

#[test]
fn channel_invalid_closed_resume() {
    assert!(
        ChannelState::Closed
            .transition(ChannelEvent::Resume)
            .is_err()
    );
}

#[test]
fn channel_invalid_closed_reject() {
    assert!(
        ChannelState::Closed
            .transition(ChannelEvent::Reject)
            .is_err()
    );
}

#[test]
fn channel_invalid_closed_reset() {
    assert!(
        ChannelState::Closed
            .transition(ChannelEvent::Reset)
            .is_err()
    );
}

#[test]
fn channel_invalid_opening_open() {
    assert!(
        ChannelState::Opening
            .transition(ChannelEvent::Open)
            .is_err()
    );
}

#[test]
fn channel_invalid_opening_close() {
    assert!(
        ChannelState::Opening
            .transition(ChannelEvent::Close)
            .is_err()
    );
}

#[test]
fn channel_invalid_opening_suspend() {
    assert!(
        ChannelState::Opening
            .transition(ChannelEvent::Suspend)
            .is_err()
    );
}

#[test]
fn channel_invalid_opening_resume() {
    assert!(
        ChannelState::Opening
            .transition(ChannelEvent::Resume)
            .is_err()
    );
}

#[test]
fn channel_invalid_opening_reset() {
    assert!(
        ChannelState::Opening
            .transition(ChannelEvent::Reset)
            .is_err()
    );
}

#[test]
fn channel_invalid_active_open() {
    assert!(ChannelState::Active.transition(ChannelEvent::Open).is_err());
}

#[test]
fn channel_invalid_active_ack() {
    assert!(ChannelState::Active.transition(ChannelEvent::Ack).is_err());
}

#[test]
fn channel_invalid_active_reject() {
    assert!(
        ChannelState::Active
            .transition(ChannelEvent::Reject)
            .is_err()
    );
}

#[test]
fn channel_invalid_active_resume() {
    assert!(
        ChannelState::Active
            .transition(ChannelEvent::Resume)
            .is_err()
    );
}

#[test]
fn channel_invalid_suspended_open() {
    assert!(
        ChannelState::Suspended
            .transition(ChannelEvent::Open)
            .is_err()
    );
}

#[test]
fn channel_invalid_suspended_ack() {
    assert!(
        ChannelState::Suspended
            .transition(ChannelEvent::Ack)
            .is_err()
    );
}

#[test]
fn channel_invalid_suspended_suspend() {
    assert!(
        ChannelState::Suspended
            .transition(ChannelEvent::Suspend)
            .is_err()
    );
}

#[test]
fn channel_invalid_suspended_reject() {
    assert!(
        ChannelState::Suspended
            .transition(ChannelEvent::Reject)
            .is_err()
    );
}

#[test]
fn channel_invalid_suspended_reset() {
    assert!(
        ChannelState::Suspended
            .transition(ChannelEvent::Reset)
            .is_err()
    );
}

#[test]
fn channel_invalid_rejected_ack() {
    assert!(
        ChannelState::Rejected
            .transition(ChannelEvent::Ack)
            .is_err()
    );
}

#[test]
fn channel_invalid_rejected_close() {
    assert!(
        ChannelState::Rejected
            .transition(ChannelEvent::Close)
            .is_err()
    );
}

#[test]
fn channel_invalid_rejected_suspend() {
    assert!(
        ChannelState::Rejected
            .transition(ChannelEvent::Suspend)
            .is_err()
    );
}

#[test]
fn channel_invalid_rejected_resume() {
    assert!(
        ChannelState::Rejected
            .transition(ChannelEvent::Resume)
            .is_err()
    );
}

#[test]
fn channel_invalid_rejected_reject() {
    assert!(
        ChannelState::Rejected
            .transition(ChannelEvent::Reject)
            .is_err()
    );
}

#[test]
fn channel_invalid_rejected_reset() {
    assert!(
        ChannelState::Rejected
            .transition(ChannelEvent::Reset)
            .is_err()
    );
}

#[test]
fn channel_invalid_transition_display() {
    let err = InvalidTransition {
        from: ChannelState::Closed,
        event: ChannelEvent::Ack,
    };
    assert_eq!(err.to_string(), "invalid channel transition: Closed + Ack");
}

#[test]
fn channel_invalid_transition_is_error() {
    let err = InvalidTransition {
        from: ChannelState::Closed,
        event: ChannelEvent::Resume,
    };
    // Ensure it implements std::error::Error.
    let _: &dyn std::error::Error = &err;
}

// ── SessionState tests ─────────────────────────────────────────

#[test]
fn session_default_is_connecting() {
    assert_eq!(SessionState::default(), SessionState::Connecting);
}

#[test]
fn session_happy_path() {
    let s = SessionState::Connecting;
    let s = s.transition(SessionEvent::TlsComplete).unwrap();
    assert_eq!(s, SessionState::Handshake);
    let s = s.transition(SessionEvent::ServerHello).unwrap();
    assert_eq!(s, SessionState::Authenticating);
    let s = s.transition(SessionEvent::LoginSuccess).unwrap();
    assert_eq!(s, SessionState::Active);
    assert!(s.is_active());
    let s = s.transition(SessionEvent::Disconnect).unwrap();
    assert_eq!(s, SessionState::Closed);
}

#[test]
fn session_login_failure() {
    let s = SessionState::Authenticating;
    let s = s.transition(SessionEvent::LoginFailure).unwrap();
    assert_eq!(s, SessionState::Disconnected);
}

#[test]
fn session_reconnect_flow() {
    let s = SessionState::Active;
    let s = s.transition(SessionEvent::ConnectionLost).unwrap();
    assert_eq!(s, SessionState::Reconnecting);
    let s = s.transition(SessionEvent::ResumeOk).unwrap();
    assert_eq!(s, SessionState::Active);
}

#[test]
fn session_reconnect_timeout() {
    let s = SessionState::Active;
    let s = s.transition(SessionEvent::ConnectionLost).unwrap();
    assert_eq!(s, SessionState::Reconnecting);
    let s = s.transition(SessionEvent::Timeout).unwrap();
    assert_eq!(s, SessionState::Disconnected);
}

#[test]
fn session_is_active_only_when_active() {
    assert!(!SessionState::Connecting.is_active());
    assert!(!SessionState::Handshake.is_active());
    assert!(!SessionState::Authenticating.is_active());
    assert!(SessionState::Active.is_active());
    assert!(!SessionState::Reconnecting.is_active());
    assert!(!SessionState::Disconnected.is_active());
    assert!(!SessionState::Closed.is_active());
}

#[test]
fn session_invalid_connecting_disconnect() {
    assert!(
        SessionState::Connecting
            .transition(SessionEvent::Disconnect)
            .is_err()
    );
}

#[test]
fn session_invalid_handshake_login_success() {
    assert!(
        SessionState::Handshake
            .transition(SessionEvent::LoginSuccess)
            .is_err()
    );
}

#[test]
fn session_invalid_active_tls_complete() {
    assert!(
        SessionState::Active
            .transition(SessionEvent::TlsComplete)
            .is_err()
    );
}

#[test]
fn session_invalid_closed_is_terminal() {
    assert!(
        SessionState::Closed
            .transition(SessionEvent::TlsComplete)
            .is_err()
    );
    assert!(
        SessionState::Closed
            .transition(SessionEvent::Disconnect)
            .is_err()
    );
}

#[test]
fn session_invalid_disconnected_resume() {
    assert!(
        SessionState::Disconnected
            .transition(SessionEvent::ResumeOk)
            .is_err()
    );
}

#[test]
fn session_invalid_reconnecting_disconnect() {
    assert!(
        SessionState::Reconnecting
            .transition(SessionEvent::Disconnect)
            .is_err()
    );
}

#[test]
fn session_invalid_transition_display() {
    let err = InvalidSessionTransition {
        from: SessionState::Closed,
        event: SessionEvent::Disconnect,
    };
    assert_eq!(
        err.to_string(),
        "invalid session transition: Closed + Disconnect"
    );
}

#[test]
fn session_invalid_transition_is_error() {
    let err = InvalidSessionTransition {
        from: SessionState::Closed,
        event: SessionEvent::TlsComplete,
    };
    let _: &dyn std::error::Error = &err;
}

// ── EmergencyState tests ───────────────────────────────────────

#[test]
fn emergency_default_is_idle() {
    assert_eq!(EmergencyState::default(), EmergencyState::Idle);
}

#[test]
fn emergency_crash_report_restart_flow() {
    let s = EmergencyState::Idle;
    let s = s.transition(EmergencyEvent::CrashDetected).unwrap();
    assert_eq!(s, EmergencyState::Crash);
    let s = s.transition(EmergencyEvent::ReportRequested).unwrap();
    assert_eq!(s, EmergencyState::StreamingReport);
    let s = s.transition(EmergencyEvent::ReportComplete).unwrap();
    assert_eq!(s, EmergencyState::Crash);
    let s = s.transition(EmergencyEvent::RestartRequested).unwrap();
    assert_eq!(s, EmergencyState::Restarting);
    let s = s.transition(EmergencyEvent::RestartSuccess).unwrap();
    assert_eq!(s, EmergencyState::Idle);
}

#[test]
fn emergency_crash_direct_restart_success() {
    let s = EmergencyState::Crash;
    let s = s.transition(EmergencyEvent::RestartSuccess).unwrap();
    assert_eq!(s, EmergencyState::Idle);
}

#[test]
fn emergency_restart_failed() {
    let s = EmergencyState::Crash;
    let s = s.transition(EmergencyEvent::RestartRequested).unwrap();
    assert_eq!(s, EmergencyState::Restarting);
    let s = s.transition(EmergencyEvent::RestartFailed).unwrap();
    assert_eq!(s, EmergencyState::Failed);
}

#[test]
fn emergency_invalid_idle_report() {
    assert!(
        EmergencyState::Idle
            .transition(EmergencyEvent::ReportRequested)
            .is_err()
    );
}

#[test]
fn emergency_invalid_idle_restart() {
    assert!(
        EmergencyState::Idle
            .transition(EmergencyEvent::RestartRequested)
            .is_err()
    );
}

#[test]
fn emergency_invalid_failed_is_terminal() {
    assert!(
        EmergencyState::Failed
            .transition(EmergencyEvent::CrashDetected)
            .is_err()
    );
    assert!(
        EmergencyState::Failed
            .transition(EmergencyEvent::RestartRequested)
            .is_err()
    );
}

#[test]
fn emergency_invalid_streaming_crash() {
    assert!(
        EmergencyState::StreamingReport
            .transition(EmergencyEvent::CrashDetected)
            .is_err()
    );
}

#[test]
fn emergency_invalid_transition_display() {
    let err = InvalidEmergencyTransition {
        from: EmergencyState::Idle,
        event: EmergencyEvent::ReportRequested,
    };
    assert_eq!(
        err.to_string(),
        "invalid emergency transition: Idle + ReportRequested"
    );
}

#[test]
fn emergency_invalid_transition_is_error() {
    let err = InvalidEmergencyTransition {
        from: EmergencyState::Failed,
        event: EmergencyEvent::CrashDetected,
    };
    let _: &dyn std::error::Error = &err;
}

// ── VideoState tests ───────────────────────────────────────────

#[test]
fn video_default_is_inactive() {
    assert_eq!(VideoState::default(), VideoState::Inactive);
}

#[test]
fn video_happy_path() {
    let s = VideoState::Inactive;
    let s = s.transition(VideoEvent::ChannelOpen).unwrap();
    assert_eq!(s, VideoState::Negotiating);
    let s = s.transition(VideoEvent::Ack).unwrap();
    assert_eq!(s, VideoState::Streaming);
    let s = s.transition(VideoEvent::Close).unwrap();
    assert_eq!(s, VideoState::Closed);
}

#[test]
fn video_codec_switch_cycle() {
    let s = VideoState::Streaming;
    let s = s.transition(VideoEvent::CodecSwitch).unwrap();
    assert_eq!(s, VideoState::Switching);
    let s = s.transition(VideoEvent::KeyFrameSent).unwrap();
    assert_eq!(s, VideoState::Streaming);
}

#[test]
fn video_suspend_resume() {
    let s = VideoState::Streaming;
    let s = s.transition(VideoEvent::Suspend).unwrap();
    assert_eq!(s, VideoState::Suspended);
    let s = s.transition(VideoEvent::Resume).unwrap();
    assert_eq!(s, VideoState::Streaming);
}

#[test]
fn video_close_from_suspended() {
    let s = VideoState::Suspended;
    let s = s.transition(VideoEvent::Close).unwrap();
    assert_eq!(s, VideoState::Closed);
}

#[test]
fn video_invalid_inactive_ack() {
    assert!(VideoState::Inactive.transition(VideoEvent::Ack).is_err());
}

#[test]
fn video_invalid_negotiating_close() {
    assert!(
        VideoState::Negotiating
            .transition(VideoEvent::Close)
            .is_err()
    );
}

#[test]
fn video_invalid_switching_close() {
    assert!(VideoState::Switching.transition(VideoEvent::Close).is_err());
}

#[test]
fn video_invalid_closed_open() {
    assert!(
        VideoState::Closed
            .transition(VideoEvent::ChannelOpen)
            .is_err()
    );
}

// ── TileState tests ────────────────────────────────────────────

#[test]
fn tile_default_is_inactive() {
    assert_eq!(TileState::default(), TileState::Inactive);
}

#[test]
fn tile_happy_path() {
    let s = TileState::Inactive;
    let s = s.transition(TileEvent::ChannelOpen).unwrap();
    assert_eq!(s, TileState::Configuring);
    let s = s.transition(TileEvent::Ack).unwrap();
    assert_eq!(s, TileState::KeyFrame);
    let s = s.transition(TileEvent::KeyFrameComplete).unwrap();
    assert_eq!(s, TileState::Streaming);
    let s = s.transition(TileEvent::Close).unwrap();
    assert_eq!(s, TileState::Closed);
}

#[test]
fn tile_keyframe_request_cycle() {
    let s = TileState::Streaming;
    let s = s.transition(TileEvent::KeyFrameRequest).unwrap();
    assert_eq!(s, TileState::KeyFrame);
    let s = s.transition(TileEvent::KeyFrameComplete).unwrap();
    assert_eq!(s, TileState::Streaming);
}

#[test]
fn tile_resize_reconfigure() {
    let s = TileState::Streaming;
    let s = s.transition(TileEvent::Resize).unwrap();
    assert_eq!(s, TileState::Reconfiguring);
    let s = s.transition(TileEvent::Ack).unwrap();
    assert_eq!(s, TileState::KeyFrame);
    let s = s.transition(TileEvent::KeyFrameComplete).unwrap();
    assert_eq!(s, TileState::Streaming);
}

#[test]
fn tile_invalid_inactive_ack() {
    assert!(TileState::Inactive.transition(TileEvent::Ack).is_err());
}

#[test]
fn tile_invalid_configuring_close() {
    assert!(TileState::Configuring.transition(TileEvent::Close).is_err());
}

#[test]
fn tile_invalid_keyframe_close() {
    assert!(TileState::KeyFrame.transition(TileEvent::Close).is_err());
}

#[test]
fn tile_invalid_closed_open() {
    assert!(
        TileState::Closed
            .transition(TileEvent::ChannelOpen)
            .is_err()
    );
}

// ── AudioState tests ───────────────────────────────────────────

#[test]
fn audio_default_is_inactive() {
    assert_eq!(AudioState::default(), AudioState::Inactive);
}

#[test]
fn audio_happy_path() {
    let s = AudioState::Inactive;
    let s = s.transition(AudioEvent::ChannelOpen).unwrap();
    assert_eq!(s, AudioState::Negotiating);
    let s = s.transition(AudioEvent::ConfigAgreed).unwrap();
    assert_eq!(s, AudioState::Streaming);
    let s = s.transition(AudioEvent::Close).unwrap();
    assert_eq!(s, AudioState::Closed);
}

#[test]
fn audio_mute_unmute_cycle() {
    let s = AudioState::Streaming;
    let s = s.transition(AudioEvent::Mute).unwrap();
    assert_eq!(s, AudioState::Muted);
    let s = s.transition(AudioEvent::Unmute).unwrap();
    assert_eq!(s, AudioState::Streaming);
}

#[test]
fn audio_close_from_muted() {
    let s = AudioState::Muted;
    let s = s.transition(AudioEvent::Close).unwrap();
    assert_eq!(s, AudioState::Closed);
}

#[test]
fn audio_invalid_inactive_mute() {
    assert!(AudioState::Inactive.transition(AudioEvent::Mute).is_err());
}

#[test]
fn audio_invalid_negotiating_close() {
    assert!(
        AudioState::Negotiating
            .transition(AudioEvent::Close)
            .is_err()
    );
}

#[test]
fn audio_invalid_streaming_unmute() {
    assert!(
        AudioState::Streaming
            .transition(AudioEvent::Unmute)
            .is_err()
    );
}

#[test]
fn audio_invalid_closed_open() {
    assert!(
        AudioState::Closed
            .transition(AudioEvent::ChannelOpen)
            .is_err()
    );
}

// ── ClipboardState tests ───────────────────────────────────────

#[test]
fn clipboard_default_is_idle() {
    assert_eq!(ClipboardState::default(), ClipboardState::Idle);
}

#[test]
fn clipboard_happy_path() {
    let s = ClipboardState::Idle;
    let s = s.transition(ClipboardEvent::OfferReceived).unwrap();
    assert_eq!(s, ClipboardState::OfferPending);
    let s = s.transition(ClipboardEvent::Request).unwrap();
    assert_eq!(s, ClipboardState::Transferring);
    let s = s.transition(ClipboardEvent::DataEnd).unwrap();
    assert_eq!(s, ClipboardState::Idle);
}

#[test]
fn clipboard_offer_timeout() {
    let s = ClipboardState::OfferPending;
    let s = s.transition(ClipboardEvent::Timeout).unwrap();
    assert_eq!(s, ClipboardState::Idle);
}

#[test]
fn clipboard_offer_clear() {
    let s = ClipboardState::OfferPending;
    let s = s.transition(ClipboardEvent::Clear).unwrap();
    assert_eq!(s, ClipboardState::Idle);
}

#[test]
fn clipboard_transfer_cancel() {
    let s = ClipboardState::Transferring;
    let s = s.transition(ClipboardEvent::Cancel).unwrap();
    assert_eq!(s, ClipboardState::Idle);
}

#[test]
fn clipboard_close_from_all_non_closed_states() {
    let s = ClipboardState::Idle
        .transition(ClipboardEvent::Close)
        .unwrap();
    assert_eq!(s, ClipboardState::Closed);

    let s = ClipboardState::OfferPending
        .transition(ClipboardEvent::Close)
        .unwrap();
    assert_eq!(s, ClipboardState::Closed);

    let s = ClipboardState::Transferring
        .transition(ClipboardEvent::Close)
        .unwrap();
    assert_eq!(s, ClipboardState::Closed);
}

#[test]
fn clipboard_invalid_idle_request() {
    assert!(
        ClipboardState::Idle
            .transition(ClipboardEvent::Request)
            .is_err()
    );
}

#[test]
fn clipboard_invalid_idle_data_end() {
    assert!(
        ClipboardState::Idle
            .transition(ClipboardEvent::DataEnd)
            .is_err()
    );
}

#[test]
fn clipboard_invalid_transferring_offer() {
    assert!(
        ClipboardState::Transferring
            .transition(ClipboardEvent::OfferReceived)
            .is_err()
    );
}

#[test]
fn clipboard_invalid_closed_offer() {
    assert!(
        ClipboardState::Closed
            .transition(ClipboardEvent::OfferReceived)
            .is_err()
    );
}

// ── InputState tests ───────────────────────────────────────────

#[test]
fn input_default_is_inactive() {
    assert_eq!(InputState::default(), InputState::Inactive);
}

#[test]
fn input_happy_path() {
    let s = InputState::Inactive;
    let s = s.transition(InputEvent::ChannelOpen).unwrap();
    assert_eq!(s, InputState::Syncing);
    let s = s.transition(InputEvent::SyncComplete).unwrap();
    assert_eq!(s, InputState::Active);
    let s = s.transition(InputEvent::Close).unwrap();
    assert_eq!(s, InputState::Closed);
}

#[test]
fn input_reconnect_resync() {
    let s = InputState::Active;
    let s = s.transition(InputEvent::Reconnect).unwrap();
    assert_eq!(s, InputState::Syncing);
    let s = s.transition(InputEvent::SyncComplete).unwrap();
    assert_eq!(s, InputState::Active);
}

#[test]
fn input_suspend_resume_resync() {
    let s = InputState::Active;
    let s = s.transition(InputEvent::Suspend).unwrap();
    assert_eq!(s, InputState::Suspended);
    let s = s.transition(InputEvent::Resume).unwrap();
    assert_eq!(s, InputState::Syncing);
    let s = s.transition(InputEvent::SyncComplete).unwrap();
    assert_eq!(s, InputState::Active);
}

#[test]
fn input_close_from_suspended() {
    let s = InputState::Suspended;
    let s = s.transition(InputEvent::Close).unwrap();
    assert_eq!(s, InputState::Closed);
}

#[test]
fn input_invalid_inactive_sync() {
    assert!(
        InputState::Inactive
            .transition(InputEvent::SyncComplete)
            .is_err()
    );
}

#[test]
fn input_invalid_syncing_close() {
    assert!(InputState::Syncing.transition(InputEvent::Close).is_err());
}

#[test]
fn input_invalid_active_open() {
    assert!(
        InputState::Active
            .transition(InputEvent::ChannelOpen)
            .is_err()
    );
}

#[test]
fn input_invalid_closed_open() {
    assert!(
        InputState::Closed
            .transition(InputEvent::ChannelOpen)
            .is_err()
    );
}

// ── CursorState tests ──────────────────────────────────────────

#[test]
fn cursor_default_is_inactive() {
    assert_eq!(CursorState::default(), CursorState::Inactive);
}

#[test]
fn cursor_happy_path() {
    let s = CursorState::Inactive;
    let s = s.transition(CursorEvent::ChannelOpen).unwrap();
    assert_eq!(s, CursorState::Active);
    let s = s.transition(CursorEvent::Close).unwrap();
    assert_eq!(s, CursorState::Closed);
}

#[test]
fn cursor_hide_show_cycle() {
    let s = CursorState::Active;
    let s = s.transition(CursorEvent::Hide).unwrap();
    assert_eq!(s, CursorState::Hidden);
    let s = s.transition(CursorEvent::Show).unwrap();
    assert_eq!(s, CursorState::Active);
}

#[test]
fn cursor_close_from_hidden() {
    let s = CursorState::Hidden;
    let s = s.transition(CursorEvent::Close).unwrap();
    assert_eq!(s, CursorState::Closed);
}

#[test]
fn cursor_invalid_inactive_hide() {
    assert!(CursorState::Inactive.transition(CursorEvent::Hide).is_err());
}

#[test]
fn cursor_invalid_inactive_show() {
    assert!(CursorState::Inactive.transition(CursorEvent::Show).is_err());
}

#[test]
fn cursor_invalid_inactive_close() {
    assert!(
        CursorState::Inactive
            .transition(CursorEvent::Close)
            .is_err()
    );
}

#[test]
fn cursor_invalid_active_show() {
    assert!(CursorState::Active.transition(CursorEvent::Show).is_err());
}

#[test]
fn cursor_invalid_hidden_hide() {
    assert!(CursorState::Hidden.transition(CursorEvent::Hide).is_err());
}

#[test]
fn cursor_invalid_closed_open() {
    assert!(
        CursorState::Closed
            .transition(CursorEvent::ChannelOpen)
            .is_err()
    );
}
