//! Tests for the IPC message bus subsystem (bus, service, match_rule, serial,
//! well_known).

use crate::bus::{BusAddress, BusMessage, BusMessageType, MessageBus, Signal};
use crate::match_rule::{MatchRule, MatchRuleBuilder};
use crate::serial::{self, BusValue, DeserializeError};
use crate::service::{
    BusError, Interface, MethodCall, MethodSignature, Response, Service, ServiceInfo,
    ServiceRegistry,
};
use crate::well_known;

// ════════════════════════════════════════════════════════════════════════
// serial.rs tests
// ════════════════════════════════════════════════════════════════════════

#[test]
fn serial_bool_roundtrip() {
    let val = BusValue::Bool(true);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, BusValue::Bool(true));
}

#[test]
fn serial_bool_false() {
    let val = BusValue::Bool(false);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, BusValue::Bool(false));
}

#[test]
fn serial_int32_roundtrip() {
    let val = BusValue::Int32(-42);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_int64_roundtrip() {
    let val = BusValue::Int64(i64::MIN);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_uint32_roundtrip() {
    let val = BusValue::Uint32(u32::MAX);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_uint64_roundtrip() {
    let val = BusValue::Uint64(0xDEAD_BEEF_CAFE_BABE);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_float64_roundtrip() {
    let val = BusValue::Float64(std::f64::consts::PI);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_string_roundtrip() {
    let val = BusValue::String("hello, bus!".into());
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_empty_string() {
    let val = BusValue::String(String::new());
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_byte_array_roundtrip() {
    let val = BusValue::ByteArray(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_array_roundtrip() {
    let val = BusValue::Array(vec![
        BusValue::Int32(1),
        BusValue::Int32(2),
        BusValue::Int32(3),
    ]);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_empty_array() {
    let val = BusValue::Array(Vec::new());
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_dict_roundtrip() {
    let val = BusValue::Dict(vec![
        ("name".into(), BusValue::String("test".into())),
        ("version".into(), BusValue::Uint32(1)),
    ]);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_nested_array() {
    let inner = BusValue::Array(vec![BusValue::Bool(true), BusValue::Bool(false)]);
    let val = BusValue::Array(vec![inner.clone(), inner]);
    let bytes = serial::serialize(&val);
    let out = serial::deserialize(&bytes).unwrap();
    assert_eq!(out, val);
}

#[test]
fn serial_truncated_data() {
    let bytes = serial::serialize(&BusValue::Int32(42));
    let truncated = &bytes[..bytes.len() - 1];
    assert_eq!(
        serial::deserialize(truncated),
        Err(DeserializeError::UnexpectedEof)
    );
}

#[test]
fn serial_empty_input() {
    assert_eq!(
        serial::deserialize(&[]),
        Err(DeserializeError::UnexpectedEof)
    );
}

#[test]
fn serial_unknown_tag() {
    assert_eq!(
        serial::deserialize(&[0xFF]),
        Err(DeserializeError::UnknownTag(0xFF))
    );
}

#[test]
fn serial_trailing_data() {
    let mut bytes = serial::serialize(&BusValue::Bool(true));
    bytes.push(0x00);
    assert_eq!(
        serial::deserialize(&bytes),
        Err(DeserializeError::TrailingData)
    );
}

#[test]
fn bus_value_type_signatures() {
    assert_eq!(BusValue::Bool(true).type_signature(), "b");
    assert_eq!(BusValue::Int32(0).type_signature(), "i");
    assert_eq!(BusValue::Int64(0).type_signature(), "x");
    assert_eq!(BusValue::Uint32(0).type_signature(), "u");
    assert_eq!(BusValue::Uint64(0).type_signature(), "t");
    assert_eq!(BusValue::Float64(0.0).type_signature(), "d");
    assert_eq!(BusValue::String("".into()).type_signature(), "s");
    assert_eq!(BusValue::ByteArray(vec![]).type_signature(), "ay");
    assert_eq!(BusValue::Dict(vec![]).type_signature(), "a{sv}");
    assert_eq!(BusValue::Array(vec![]).type_signature(), "av");
    assert_eq!(
        BusValue::Array(vec![BusValue::Int32(1)]).type_signature(),
        "ai"
    );
}

#[test]
fn bus_value_accessors() {
    assert_eq!(BusValue::String("hi".into()).as_str(), Some("hi"));
    assert_eq!(BusValue::Int32(7).as_str(), None);
    assert_eq!(BusValue::Int32(7).as_i32(), Some(7));
    assert_eq!(BusValue::String("x".into()).as_i32(), None);
    assert_eq!(BusValue::Bool(true).as_bool(), Some(true));
    assert_eq!(BusValue::Int32(0).as_bool(), None);
}

#[test]
fn bus_value_is_container() {
    assert!(BusValue::Array(vec![]).is_container());
    assert!(BusValue::Dict(vec![]).is_container());
    assert!(!BusValue::Int32(0).is_container());
    assert!(!BusValue::String("".into()).is_container());
}

// ════════════════════════════════════════════════════════════════════════
// match_rule.rs tests
// ════════════════════════════════════════════════════════════════════════

#[test]
fn match_rule_empty_matches_everything() {
    let rule = MatchRule::new();
    assert!(rule.is_empty());
    assert!(rule.matches("any.sender", "any.iface", "AnyMember", "/any/path", None));
}

#[test]
fn match_rule_sender_filter() {
    let rule = MatchRuleBuilder::new()
        .sender("org.liquide.Shell")
        .build();
    assert!(rule.matches("org.liquide.Shell", "i", "m", "/", None));
    assert!(!rule.matches("org.liquide.Audio", "i", "m", "/", None));
}

#[test]
fn match_rule_interface_filter() {
    let rule = MatchRuleBuilder::new()
        .interface("org.liquide.Shell")
        .build();
    assert!(rule.matches("s", "org.liquide.Shell", "m", "/", None));
    assert!(!rule.matches("s", "org.liquide.Audio", "m", "/", None));
}

#[test]
fn match_rule_member_filter() {
    let rule = MatchRuleBuilder::new().member("WindowOpened").build();
    assert!(rule.matches("s", "i", "WindowOpened", "/", None));
    assert!(!rule.matches("s", "i", "WindowClosed", "/", None));
}

#[test]
fn match_rule_path_filter() {
    let rule = MatchRuleBuilder::new().path("/desktop").build();
    assert!(rule.matches("s", "i", "m", "/desktop", None));
    assert!(!rule.matches("s", "i", "m", "/windows", None));
}

#[test]
fn match_rule_arg0_filter() {
    let rule = MatchRuleBuilder::new().arg0("dark").build();
    assert!(rule.matches("s", "i", "m", "/", Some("dark")));
    assert!(!rule.matches("s", "i", "m", "/", Some("light")));
    assert!(!rule.matches("s", "i", "m", "/", None));
}

#[test]
fn match_rule_combined_filters() {
    let rule = MatchRuleBuilder::new()
        .sender("org.liquide.Shell")
        .interface("org.liquide.Shell")
        .member("FocusChanged")
        .build();
    assert!(rule.matches(
        "org.liquide.Shell",
        "org.liquide.Shell",
        "FocusChanged",
        "/",
        None
    ));
    // Wrong member
    assert!(!rule.matches(
        "org.liquide.Shell",
        "org.liquide.Shell",
        "WindowOpened",
        "/",
        None
    ));
    // Wrong sender
    assert!(!rule.matches(
        "org.liquide.Audio",
        "org.liquide.Shell",
        "FocusChanged",
        "/",
        None
    ));
}

#[test]
fn match_rule_to_string() {
    let rule = MatchRuleBuilder::new()
        .sender("org.liquide.Shell")
        .member("Ping")
        .build();
    let s = rule.to_rule_string();
    assert!(s.contains("sender='org.liquide.Shell'"));
    assert!(s.contains("member='Ping'"));
}

#[test]
fn match_rule_default_is_empty() {
    let rule: MatchRule = Default::default();
    assert!(rule.is_empty());
}

// ════════════════════════════════════════════════════════════════════════
// service.rs tests
// ════════════════════════════════════════════════════════════════════════

/// A trivial service for testing.
struct EchoService;

impl Service for EchoService {
    fn handle_method(&mut self, call: &MethodCall) -> Result<Response, BusError> {
        match call.member.as_str() {
            "Echo" => Ok(Response::single(
                call.args.first().cloned().unwrap_or(BusValue::Bool(false)),
            )),
            "Add" => {
                let a = call
                    .args
                    .first()
                    .and_then(|v| v.as_i32())
                    .ok_or_else(|| BusError::invalid_args("expected int32 arg[0]"))?;
                let b = call
                    .args
                    .get(1)
                    .and_then(|v| v.as_i32())
                    .ok_or_else(|| BusError::invalid_args("expected int32 arg[1]"))?;
                Ok(Response::single(BusValue::Int32(a + b)))
            }
            "Void" => Ok(Response::empty()),
            _ => Err(BusError::unknown_method(&call.member)),
        }
    }

    fn info(&self) -> ServiceInfo {
        ServiceInfo::new("org.liquide.Echo", "0.1.0").with_interface(
            Interface::new("org.liquide.Echo")
                .with_method(MethodSignature::new("Echo", "v", "v"))
                .with_method(MethodSignature::new("Add", "ii", "i"))
                .with_method(MethodSignature::new("Void", "", "")),
        )
    }
}

#[test]
fn service_registry_register_and_call() {
    let mut reg = ServiceRegistry::new();
    assert!(reg.register(Box::new(EchoService)));
    assert_eq!(reg.count(), 1);
    assert!(reg.contains("org.liquide.Echo"));

    let call = MethodCall {
        sender: "test".into(),
        interface: "org.liquide.Echo".into(),
        member: "Echo".into(),
        path: "/".into(),
        args: vec![BusValue::String("hello".into())],
    };
    let resp = reg.call("org.liquide.Echo", &call).unwrap();
    assert_eq!(resp.values, vec![BusValue::String("hello".into())]);
}

#[test]
fn service_registry_duplicate_register() {
    let mut reg = ServiceRegistry::new();
    assert!(reg.register(Box::new(EchoService)));
    // Second registration should fail.
    assert!(!reg.register(Box::new(EchoService)));
    assert_eq!(reg.count(), 1);
}

#[test]
fn service_registry_unregister() {
    let mut reg = ServiceRegistry::new();
    reg.register(Box::new(EchoService));
    assert!(reg.unregister("org.liquide.Echo"));
    assert_eq!(reg.count(), 0);
    assert!(!reg.unregister("org.liquide.Echo")); // already gone
}

#[test]
fn service_unknown_method() {
    let mut reg = ServiceRegistry::new();
    reg.register(Box::new(EchoService));

    let call = MethodCall {
        sender: "test".into(),
        interface: "org.liquide.Echo".into(),
        member: "Nonexistent".into(),
        path: "/".into(),
        args: vec![],
    };
    let err = reg.call("org.liquide.Echo", &call).unwrap_err();
    assert!(err.name.contains("UnknownMethod"));
}

#[test]
fn service_unknown_service() {
    let mut reg = ServiceRegistry::new();
    let call = MethodCall {
        sender: "test".into(),
        interface: "x".into(),
        member: "y".into(),
        path: "/".into(),
        args: vec![],
    };
    let err = reg.call("org.liquide.Missing", &call).unwrap_err();
    assert!(err.name.contains("ServiceUnknown"));
}

#[test]
fn service_introspect() {
    let mut reg = ServiceRegistry::new();
    reg.register(Box::new(EchoService));

    let info = reg.introspect("org.liquide.Echo").unwrap();
    assert_eq!(info.name, "org.liquide.Echo");
    assert_eq!(info.version, "0.1.0");

    let iface = info.find_interface("org.liquide.Echo").unwrap();
    assert_eq!(iface.methods.len(), 3);
    assert!(iface.find_method("Echo").is_some());
    assert!(iface.find_method("Add").is_some());
    assert!(iface.find_method("Void").is_some());
    assert!(iface.find_method("Missing").is_none());
}

#[test]
fn service_introspect_missing() {
    let reg = ServiceRegistry::new();
    assert!(reg.introspect("org.liquide.Nope").is_none());
}

#[test]
fn service_list_names() {
    let mut reg = ServiceRegistry::new();
    reg.register(Box::new(EchoService));
    let names = reg.list_names();
    assert!(names.contains(&"org.liquide.Echo".to_owned()));
}

#[test]
fn service_add_method_result() {
    let mut reg = ServiceRegistry::new();
    reg.register(Box::new(EchoService));

    let call = MethodCall {
        sender: "test".into(),
        interface: "org.liquide.Echo".into(),
        member: "Add".into(),
        path: "/".into(),
        args: vec![BusValue::Int32(3), BusValue::Int32(4)],
    };
    let resp = reg.call("org.liquide.Echo", &call).unwrap();
    assert_eq!(resp.values, vec![BusValue::Int32(7)]);
}

#[test]
fn service_add_invalid_args() {
    let mut reg = ServiceRegistry::new();
    reg.register(Box::new(EchoService));

    let call = MethodCall {
        sender: "test".into(),
        interface: "org.liquide.Echo".into(),
        member: "Add".into(),
        path: "/".into(),
        args: vec![BusValue::String("oops".into())],
    };
    let err = reg.call("org.liquide.Echo", &call).unwrap_err();
    assert!(err.name.contains("InvalidArgs"));
}

#[test]
fn service_void_method() {
    let mut reg = ServiceRegistry::new();
    reg.register(Box::new(EchoService));

    let call = MethodCall {
        sender: "test".into(),
        interface: "org.liquide.Echo".into(),
        member: "Void".into(),
        path: "/".into(),
        args: vec![],
    };
    let resp = reg.call("org.liquide.Echo", &call).unwrap();
    assert!(resp.values.is_empty());
}

#[test]
fn bus_error_display() {
    let err = BusError::new("org.liquide.Error.Test", "something went wrong");
    let s = format!("{err}");
    assert!(s.contains("org.liquide.Error.Test"));
    assert!(s.contains("something went wrong"));
}

#[test]
fn response_constructors() {
    let empty = Response::empty();
    assert!(empty.values.is_empty());

    let single = Response::single(BusValue::Bool(true));
    assert_eq!(single.values.len(), 1);

    let many = Response::many(vec![BusValue::Int32(1), BusValue::Int32(2)]);
    assert_eq!(many.values.len(), 2);
}

#[test]
fn method_signature_construction() {
    let sig = MethodSignature::new("Ping", "", "b");
    assert_eq!(sig.name, "Ping");
    assert_eq!(sig.in_signature, "");
    assert_eq!(sig.out_signature, "b");
}

#[test]
fn interface_find_method() {
    let iface = Interface::new("org.test.Iface")
        .with_method(MethodSignature::new("Foo", "s", "s"))
        .with_method(MethodSignature::new("Bar", "i", ""));
    assert!(iface.find_method("Foo").is_some());
    assert!(iface.find_method("Bar").is_some());
    assert!(iface.find_method("Baz").is_none());
}

#[test]
fn service_info_find_interface() {
    let info = ServiceInfo::new("test", "1.0")
        .with_interface(Interface::new("org.test.A"))
        .with_interface(Interface::new("org.test.B"));
    assert!(info.find_interface("org.test.A").is_some());
    assert!(info.find_interface("org.test.B").is_some());
    assert!(info.find_interface("org.test.C").is_none());
}

// ════════════════════════════════════════════════════════════════════════
// bus.rs tests
// ════════════════════════════════════════════════════════════════════════

#[test]
fn bus_address_valid() {
    assert!(BusAddress::new("org.liquide.Shell").is_valid());
    assert!(BusAddress::new("com.example.Foo").is_valid());
    assert!(BusAddress::new("a.b").is_valid());
    assert!(BusAddress::new("_a._b").is_valid());
}

#[test]
fn bus_address_invalid() {
    assert!(!BusAddress::new("noDots").is_valid());
    assert!(!BusAddress::new("").is_valid());
    assert!(!BusAddress::new(".leading.dot").is_valid());
    assert!(!BusAddress::new("trailing.dot.").is_valid());
    assert!(!BusAddress::new("1org.bad").is_valid());
}

#[test]
fn bus_address_display() {
    let addr = BusAddress::new("org.liquide.Shell");
    assert_eq!(format!("{addr}"), "org.liquide.Shell");
}

#[test]
fn bus_address_from_str() {
    let addr: BusAddress = "org.liquide.Shell".into();
    assert_eq!(addr.as_str(), "org.liquide.Shell");
}

#[test]
fn bus_request_and_release_name() {
    let mut bus = MessageBus::new();
    assert!(bus.request_name("org.liquide.Shell", "shell"));
    assert!(bus.has_name("org.liquide.Shell"));
    assert_eq!(bus.name_owner("org.liquide.Shell"), Some("shell"));

    // Duplicate request fails.
    assert!(!bus.request_name("org.liquide.Shell", "imposter"));

    // Release.
    assert!(bus.release_name("org.liquide.Shell"));
    assert!(!bus.has_name("org.liquide.Shell"));
    // Can now re-claim.
    assert!(bus.request_name("org.liquide.Shell", "shell2"));
}

#[test]
fn bus_list_names() {
    let mut bus = MessageBus::new();
    bus.request_name("org.liquide.A", "a");
    bus.request_name("org.liquide.B", "b");
    let names = bus.list_names();
    assert_eq!(names.len(), 2);
}

#[test]
fn bus_subscribe_and_publish() {
    let mut bus = MessageBus::new();
    let rule = MatchRuleBuilder::new()
        .interface("org.liquide.Shell")
        .member("WindowOpened")
        .build();
    let sub_id = bus.subscribe("listener", rule);

    // Publish a matching signal.
    bus.publish(Signal::new(
        "org.liquide.Shell",
        "/desktop",
        "org.liquide.Shell",
        "WindowOpened",
    ));

    // Publish a non-matching signal.
    bus.publish(Signal::new(
        "org.liquide.Audio",
        "/audio",
        "org.liquide.Audio",
        "VolumeChanged",
    ));

    let signals = bus.drain_signals(sub_id);
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].member, "WindowOpened");
}

#[test]
fn bus_subscribe_wildcard() {
    let mut bus = MessageBus::new();
    let sub_id = bus.subscribe("listener", MatchRule::new());

    bus.publish(Signal::new("a", "/", "i", "m1"));
    bus.publish(Signal::new("b", "/", "i", "m2"));

    let signals = bus.drain_signals(sub_id);
    assert_eq!(signals.len(), 2);
}

#[test]
fn bus_unsubscribe() {
    let mut bus = MessageBus::new();
    let sub_id = bus.subscribe("listener", MatchRule::new());
    assert_eq!(bus.subscription_count(), 1);

    assert!(bus.unsubscribe(sub_id));
    assert_eq!(bus.subscription_count(), 0);
    assert!(!bus.unsubscribe(sub_id)); // already gone
}

#[test]
fn bus_unsubscribe_all_by_owner() {
    let mut bus = MessageBus::new();
    bus.subscribe("alice", MatchRule::new());
    bus.subscribe("alice", MatchRuleBuilder::new().member("X").build());
    bus.subscribe("bob", MatchRule::new());

    bus.unsubscribe_all("alice");
    assert_eq!(bus.subscription_count(), 1); // only bob's remains
}

#[test]
fn bus_drain_clears_pending() {
    let mut bus = MessageBus::new();
    let sub = bus.subscribe("l", MatchRule::new());

    bus.publish(Signal::new("s", "/", "i", "m"));
    assert_eq!(bus.pending_count(sub), 1);

    let _ = bus.drain_signals(sub);
    assert_eq!(bus.pending_count(sub), 0);
}

#[test]
fn bus_signal_log() {
    let mut bus = MessageBus::with_log_capacity(3);
    for i in 0..5 {
        bus.publish(Signal::new("s", "/", "i", &format!("sig{i}")));
    }
    let log = bus.signal_log();
    assert_eq!(log.len(), 3); // capacity=3
    assert_eq!(log[0].member, "sig2");
    assert_eq!(log[2].member, "sig4");
}

#[test]
fn bus_call_service() {
    let mut bus = MessageBus::new();
    bus.services_mut().register(Box::new(EchoService));

    let resp = bus
        .call(
            "client",
            "org.liquide.Echo",
            "/",
            "org.liquide.Echo",
            "Echo",
            vec![BusValue::String("ping".into())],
        )
        .unwrap();
    assert_eq!(resp.values, vec![BusValue::String("ping".into())]);
}

#[test]
fn bus_call_unknown_service() {
    let mut bus = MessageBus::new();
    let err = bus
        .call("client", "org.liquide.Missing", "/", "i", "m", vec![])
        .unwrap_err();
    assert!(err.name.contains("ServiceUnknown"));
}

#[test]
fn bus_call_message() {
    let mut bus = MessageBus::new();
    bus.services_mut().register(Box::new(EchoService));

    let msg = BusMessage::method_call(
        "client",
        "org.liquide.Echo",
        "/",
        "org.liquide.Echo",
        "Add",
    )
    .with_body(vec![BusValue::Int32(10), BusValue::Int32(20)]);

    let resp = bus.call_message(&msg).unwrap();
    assert_eq!(resp.values, vec![BusValue::Int32(30)]);
}

#[test]
fn bus_next_serial() {
    let mut bus = MessageBus::new();
    assert_eq!(bus.next_serial(), 1);
    assert_eq!(bus.next_serial(), 2);
    assert_eq!(bus.next_serial(), 3);
}

#[test]
fn bus_introspect() {
    let mut bus = MessageBus::new();
    bus.services_mut().register(Box::new(EchoService));

    let info = bus.introspect("org.liquide.Echo").unwrap();
    assert_eq!(info.name, "org.liquide.Echo");
    assert!(bus.introspect("org.liquide.Missing").is_none());
}

#[test]
fn bus_list_services() {
    let mut bus = MessageBus::new();
    bus.services_mut().register(Box::new(EchoService));
    let names = bus.list_services();
    assert!(names.contains(&"org.liquide.Echo".to_owned()));
}

#[test]
fn bus_disconnect() {
    let mut bus = MessageBus::new();
    bus.request_name("org.liquide.Echo", "org.liquide.Echo");
    bus.services_mut().register(Box::new(EchoService));
    let _sub = bus.subscribe("org.liquide.Echo", MatchRule::new());

    bus.disconnect("org.liquide.Echo");

    assert!(!bus.has_name("org.liquide.Echo"));
    assert_eq!(bus.subscription_count(), 0);
    assert!(!bus.services().contains("org.liquide.Echo"));
}

#[test]
fn bus_debug_format() {
    let bus = MessageBus::new();
    let debug = format!("{bus:?}");
    assert!(debug.contains("MessageBus"));
}

#[test]
fn bus_signal_with_args() {
    let mut bus = MessageBus::new();
    let rule = MatchRuleBuilder::new().arg0("dark").build();
    let sub = bus.subscribe("listener", rule);

    // Matching arg0.
    bus.publish(
        Signal::new("s", "/", "org.liquide.Settings", "ThemeChanged")
            .with_arg(BusValue::String("dark".into())),
    );
    // Non-matching arg0.
    bus.publish(
        Signal::new("s", "/", "org.liquide.Settings", "ThemeChanged")
            .with_arg(BusValue::String("light".into())),
    );

    let signals = bus.drain_signals(sub);
    assert_eq!(signals.len(), 1);
    assert_eq!(
        signals[0].args[0],
        BusValue::String("dark".into())
    );
}

#[test]
fn bus_multiple_subscribers() {
    let mut bus = MessageBus::new();
    let sub1 = bus.subscribe("a", MatchRuleBuilder::new().member("X").build());
    let sub2 = bus.subscribe("b", MatchRuleBuilder::new().member("Y").build());
    let sub3 = bus.subscribe("c", MatchRule::new()); // wildcard

    bus.publish(Signal::new("s", "/", "i", "X"));
    bus.publish(Signal::new("s", "/", "i", "Y"));

    assert_eq!(bus.drain_signals(sub1).len(), 1);
    assert_eq!(bus.drain_signals(sub2).len(), 1);
    assert_eq!(bus.drain_signals(sub3).len(), 2); // catches both
}

// ── BusMessage construction ─────────────────────────────────────────────

#[test]
fn bus_message_signal_construction() {
    let msg = BusMessage::signal("sender", "/path", "iface", "member")
        .with_arg(BusValue::Bool(true));
    assert_eq!(msg.msg_type, BusMessageType::Signal);
    assert_eq!(msg.sender, "sender");
    assert_eq!(msg.destination, "");
    assert_eq!(msg.body.len(), 1);
}

#[test]
fn bus_message_method_call_construction() {
    let msg = BusMessage::method_call("s", "d", "/p", "i", "m")
        .with_body(vec![BusValue::Int32(42)]);
    assert_eq!(msg.msg_type, BusMessageType::MethodCall);
    assert_eq!(msg.destination, "d");
    assert_eq!(msg.body, vec![BusValue::Int32(42)]);
}

#[test]
fn signal_builder() {
    let sig = Signal::new("s", "/", "i", "m")
        .with_args(vec![BusValue::Int32(1)])
        .with_arg(BusValue::Int32(2));
    assert_eq!(sig.args.len(), 2);
}

// ════════════════════════════════════════════════════════════════════════
// well_known.rs tests
// ════════════════════════════════════════════════════════════════════════

#[test]
fn well_known_service_names_are_valid_addresses() {
    let names = [
        well_known::SHELL_SERVICE,
        well_known::SETTINGS_SERVICE,
        well_known::NOTIFICATION_SERVICE,
        well_known::POWER_SERVICE,
        well_known::NETWORK_SERVICE,
        well_known::AUDIO_SERVICE,
        well_known::SESSION_SERVICE,
        well_known::FILES_SERVICE,
        well_known::ACCESSIBILITY_SERVICE,
        well_known::CLIPBOARD_SERVICE,
    ];
    for name in &names {
        assert!(
            BusAddress::new(*name).is_valid(),
            "{name} should be a valid bus address"
        );
    }
}

#[test]
fn well_known_interfaces_are_non_empty() {
    assert!(!well_known::INTROSPECTABLE_INTERFACE.is_empty());
    assert!(!well_known::PROPERTIES_INTERFACE.is_empty());
    assert!(!well_known::PEER_INTERFACE.is_empty());
}

#[test]
fn well_known_shell_methods_defined() {
    assert!(!well_known::SHELL_LIST_WINDOWS.is_empty());
    assert!(!well_known::SHELL_ACTIVATE_WINDOW.is_empty());
    assert!(!well_known::SHELL_CLOSE_WINDOW.is_empty());
}

#[test]
fn well_known_notification_methods_defined() {
    assert!(!well_known::NOTIFY_POST.is_empty());
    assert!(!well_known::NOTIFY_CLOSE.is_empty());
    assert!(!well_known::NOTIFY_GET_CAPABILITIES.is_empty());
}

#[test]
fn well_known_audio_methods_defined() {
    assert!(!well_known::AUDIO_GET_VOLUME.is_empty());
    assert!(!well_known::AUDIO_SET_VOLUME.is_empty());
    assert!(!well_known::AUDIO_GET_MUTE.is_empty());
    assert!(!well_known::AUDIO_SET_MUTE.is_empty());
}

#[test]
fn well_known_power_methods_defined() {
    assert!(!well_known::POWER_SUSPEND.is_empty());
    assert!(!well_known::POWER_SHUTDOWN.is_empty());
    assert!(!well_known::POWER_GET_BATTERY.is_empty());
}

// ════════════════════════════════════════════════════════════════════════
// Integration: end-to-end bus usage
// ════════════════════════════════════════════════════════════════════════

#[test]
fn integration_service_call_and_signal_flow() {
    let mut bus = MessageBus::new();

    // Register a service.
    bus.services_mut().register(Box::new(EchoService));
    bus.request_name("org.liquide.Echo", "echo-component");

    // A listener subscribes to echo-related signals.
    let sub = bus.subscribe(
        "listener",
        MatchRuleBuilder::new()
            .sender("org.liquide.Echo")
            .build(),
    );

    // Client calls a method.
    let resp = bus
        .call(
            "client",
            "org.liquide.Echo",
            "/",
            "org.liquide.Echo",
            "Add",
            vec![BusValue::Int32(100), BusValue::Int32(200)],
        )
        .unwrap();
    assert_eq!(resp.values, vec![BusValue::Int32(300)]);

    // Service publishes a signal after doing work.
    bus.publish(
        Signal::new(
            "org.liquide.Echo",
            "/",
            "org.liquide.Echo",
            "WorkDone",
        )
        .with_arg(BusValue::String("complete".into())),
    );

    // Listener drains.
    let signals = bus.drain_signals(sub);
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].member, "WorkDone");
}
