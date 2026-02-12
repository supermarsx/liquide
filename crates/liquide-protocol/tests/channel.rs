use liquide_protocol::channel::*;

#[test]
fn channel_class_fixed() {
    assert_eq!(ChannelId::CONTROL.class(), ChannelClass::Fixed);
    assert_eq!(ChannelId::EMERGENCY.class(), ChannelClass::Fixed);
    assert_eq!(ChannelId::INPUT.class(), ChannelClass::Fixed);
    assert_eq!(ChannelId::CURSOR.class(), ChannelClass::Fixed);
}

#[test]
fn channel_class_standard() {
    assert_eq!(ChannelId::VIDEO.class(), ChannelClass::Standard);
    assert_eq!(ChannelId::TILE.class(), ChannelClass::Standard);
    assert_eq!(ChannelId::AUDIO_PLAYBACK.class(), ChannelClass::Standard);
    assert_eq!(ChannelId::CLIPBOARD.class(), ChannelClass::Standard);
}

#[test]
fn channel_class_virtual() {
    assert_eq!(ChannelId(0xF0).class(), ChannelClass::Virtual);
    assert_eq!(ChannelId(0xF5).class(), ChannelClass::Virtual);
    assert_eq!(ChannelId(0xFE).class(), ChannelClass::Virtual);
    assert!(ChannelId(0xF0).is_virtual());
}

#[test]
fn channel_properties() {
    let props = ChannelId::CONTROL.properties().unwrap();
    assert_eq!(props.name, "Control");
    assert!(props.reliable);
    assert_eq!(props.priority, Priority::Highest);

    let props = ChannelId::VIDEO.properties().unwrap();
    assert!(!props.reliable);
    assert_eq!(props.direction, Direction::ServerToClient);
}

#[test]
fn channel_display() {
    assert_eq!(format!("{}", ChannelId::CONTROL), "Control(0x00)");
    assert_eq!(format!("{}", ChannelId::VIDEO), "Video(0x10)");
    assert_eq!(format!("{}", ChannelId(0x99)), "Unknown(0x99)");
}

#[test]
fn transport_binding() {
    assert_eq!(ChannelId::CONTROL.tcp_udp_binding(), TransportBinding::Tcp);
    assert_eq!(ChannelId::VIDEO.tcp_udp_binding(), TransportBinding::Udp);
    assert_eq!(ChannelId::CURSOR.tcp_udp_binding(), TransportBinding::Udp);
    assert_eq!(ChannelId::INPUT.tcp_udp_binding(), TransportBinding::Tcp);
}
