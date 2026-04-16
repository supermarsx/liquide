use crate::ipc::{IpcChannel, SessionEvent, SupervisorCommand};
use crate::state::SessionState;

#[test]
fn send_event_is_received_by_supervisor() {
    let (channel, handle) = IpcChannel::create("/tmp/test.sock".into());

    let event = SessionEvent::HeartbeatSent;
    channel.send_event(&event).unwrap();

    let received = handle.try_recv_event().unwrap();
    assert!(received.is_some());
    assert!(matches!(received.unwrap(), SessionEvent::HeartbeatSent));
}

#[test]
fn receive_command_from_supervisor() {
    let (channel, handle) = IpcChannel::create("/tmp/test.sock".into());

    handle.send_command(SupervisorCommand::Lock).unwrap();

    let cmd = channel.receive_command().unwrap();
    assert!(cmd.is_some());
    assert!(matches!(cmd.unwrap(), SupervisorCommand::Lock));
}

#[test]
fn receive_command_returns_none_when_empty() {
    let (channel, _handle) = IpcChannel::create("/tmp/test.sock".into());

    let cmd = channel.receive_command().unwrap();
    assert!(cmd.is_none());
}

#[test]
fn try_recv_event_returns_none_when_empty() {
    let (_channel, handle) = IpcChannel::create("/tmp/test.sock".into());

    let event = handle.try_recv_event().unwrap();
    assert!(event.is_none());
}

#[test]
fn multiple_events_arrive_in_order() {
    let (channel, handle) = IpcChannel::create("/tmp/test.sock".into());

    channel.send_event(&SessionEvent::HeartbeatSent).unwrap();
    channel
        .send_event(&SessionEvent::StateChanged {
            from: SessionState::Running,
            to: SessionState::Locked,
        })
        .unwrap();

    let first = handle.try_recv_event().unwrap().unwrap();
    assert!(matches!(first, SessionEvent::HeartbeatSent));

    let second = handle.try_recv_event().unwrap().unwrap();
    assert!(matches!(second, SessionEvent::StateChanged { .. }));
}

#[test]
fn multiple_commands_arrive_in_order() {
    let (channel, handle) = IpcChannel::create("/tmp/test.sock".into());

    handle.send_command(SupervisorCommand::Lock).unwrap();
    handle.send_command(SupervisorCommand::Unlock).unwrap();
    handle.send_command(SupervisorCommand::Shutdown).unwrap();

    let c1 = channel.receive_command().unwrap().unwrap();
    assert!(matches!(c1, SupervisorCommand::Lock));

    let c2 = channel.receive_command().unwrap().unwrap();
    assert!(matches!(c2, SupervisorCommand::Unlock));

    let c3 = channel.receive_command().unwrap().unwrap();
    assert!(matches!(c3, SupervisorCommand::Shutdown));
}

#[test]
fn send_event_errors_when_receiver_dropped() {
    let (channel, handle) = IpcChannel::create("/tmp/test.sock".into());
    drop(handle);

    let result = channel.send_event(&SessionEvent::HeartbeatSent);
    assert!(result.is_err());
}

#[test]
fn receive_command_errors_when_sender_dropped() {
    let (channel, handle) = IpcChannel::create("/tmp/test.sock".into());
    drop(handle);

    // Drain any buffered commands first.
    while let Ok(Some(_)) = channel.receive_command() {}

    let result = channel.receive_command();
    assert!(result.is_err());
}

#[test]
fn socket_path_is_preserved() {
    let (channel, _handle) = IpcChannel::create("/run/liquide/session-42.sock".into());
    assert_eq!(channel.socket_path(), "/run/liquide/session-42.sock");
}

#[test]
fn supervisor_blocking_recv_event() {
    let (channel, handle) = IpcChannel::create("/tmp/test.sock".into());

    // Send from another thread so recv_event has something to return.
    std::thread::spawn(move || {
        channel.send_event(&SessionEvent::HeartbeatSent).unwrap();
    });

    let event = handle.recv_event().unwrap();
    assert!(matches!(event, SessionEvent::HeartbeatSent));
}
