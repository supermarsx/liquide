use crate::chat::ChatChannel;

#[test]
fn test_new_channel_empty() {
    let ch = ChatChannel::new("sess-1".into());
    assert_eq!(ch.message_count(), 0);
    assert!(ch.messages().is_empty());
}

#[test]
fn test_send_message() {
    let mut ch = ChatChannel::new("sess-1".into());
    let msg = ch.send("Alice".into(), "Hello".into(), 1000);
    assert_eq!(msg.sender, "Alice");
    assert_eq!(msg.text, "Hello");
    assert_eq!(msg.sequence, 0);
    assert_eq!(ch.message_count(), 1);
}

#[test]
fn test_send_multiple_messages() {
    let mut ch = ChatChannel::new("sess-1".into());
    ch.send("Alice".into(), "Hello".into(), 1000);
    ch.send("Bob".into(), "Hi".into(), 1001);
    ch.send("Alice".into(), "How are you?".into(), 1002);
    assert_eq!(ch.message_count(), 3);
    assert_eq!(ch.messages()[2].sequence, 2);
}

#[test]
fn test_messages_since() {
    let mut ch = ChatChannel::new("sess-1".into());
    ch.send("Alice".into(), "msg0".into(), 1000);
    ch.send("Bob".into(), "msg1".into(), 1001);
    ch.send("Alice".into(), "msg2".into(), 1002);

    let since = ch.messages_since(1);
    assert_eq!(since.len(), 2);
    assert_eq!(since[0].text, "msg1");
}

#[test]
fn test_session_id() {
    let ch = ChatChannel::new("my-session".into());
    assert_eq!(ch.shadow_session_id(), "my-session");
}
