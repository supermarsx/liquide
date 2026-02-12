//! Tests for `network` submodule types.

use liquide_apps_task_manager::network::*;
use liquide_apps_task_manager::network::connection::*;
use liquide_apps_task_manager::network::dns::*;
use liquide_apps_task_manager::network::firewall::*;
use liquide_apps_task_manager::network::interface::*;
use liquide_apps_task_manager::network::bandwidth::*;
use liquide_apps_task_manager::network::topology::*;
use liquide_apps_task_manager::network::capture::*;
use liquide_apps_task_manager::network::protocol::*;

// ---------------------------------------------------------------------------
// NetworkView
// ---------------------------------------------------------------------------

#[test]
fn network_view_all_variants() {
    let variants = [
        NetworkView::Connections,
        NetworkView::DnsQueries,
        NetworkView::Protocols,
        NetworkView::Firewall,
        NetworkView::Bandwidth,
        NetworkView::Interfaces,
        NetworkView::Topology,
        NetworkView::Capture,
        NetworkView::Diagnostics,
        NetworkView::Overview,
    ];
    assert_eq!(variants.len(), 10);
}

// ---------------------------------------------------------------------------
// NetworkProtocol
// ---------------------------------------------------------------------------

#[test]
fn network_protocol_all_variants() {
    let variants = [
        NetworkProtocol::Tcp,
        NetworkProtocol::Udp,
        NetworkProtocol::Tcp6,
        NetworkProtocol::Udp6,
        NetworkProtocol::Sctp,
        NetworkProtocol::Quic,
    ];
    assert_eq!(variants.len(), 6);
}

#[test]
fn network_protocol_serde_roundtrip() {
    let val = NetworkProtocol::Sctp;
    let json = serde_json::to_string(&val).unwrap();
    let back: NetworkProtocol = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// ConnectionState
// ---------------------------------------------------------------------------

#[test]
fn connection_state_all_variants() {
    let variants = [
        ConnectionState::Established,
        ConnectionState::Listen,
        ConnectionState::TimeWait,
        ConnectionState::CloseWait,
        ConnectionState::FinWait1,
        ConnectionState::FinWait2,
        ConnectionState::Closing,
        ConnectionState::LastAck,
        ConnectionState::SynSent,
        ConnectionState::SynReceived,
        ConnectionState::Closed,
    ];
    assert_eq!(variants.len(), 11);
}

// ---------------------------------------------------------------------------
// TlsVersion
// ---------------------------------------------------------------------------

#[test]
fn tls_version_all_variants() {
    let variants = [
        TlsVersion::Tls10,
        TlsVersion::Tls11,
        TlsVersion::Tls12,
        TlsVersion::Tls13,
        TlsVersion::Dtls12,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// ConnectionAction
// ---------------------------------------------------------------------------

#[test]
fn connection_action_all_variants() {
    let variants = [
        ConnectionAction::Close,
        ConnectionAction::Block,
        ConnectionAction::AllowPermanent,
        ConnectionAction::CopyDetails,
        ConnectionAction::Whois,
        ConnectionAction::Traceroute,
        ConnectionAction::GeoLookup,
        ConnectionAction::AddFirewallRule,
        ConnectionAction::CaptureTraffic,
    ];
    assert_eq!(variants.len(), 9);
}

// ---------------------------------------------------------------------------
// DnsQueryType
// ---------------------------------------------------------------------------

#[test]
fn dns_query_type_all_variants() {
    let variants = [
        DnsQueryType::A,
        DnsQueryType::Aaaa,
        DnsQueryType::Cname,
        DnsQueryType::Mx,
        DnsQueryType::Ns,
        DnsQueryType::Txt,
        DnsQueryType::Srv,
        DnsQueryType::Ptr,
    ];
    assert_eq!(variants.len(), 8);
}

// ---------------------------------------------------------------------------
// DnsProtocol
// ---------------------------------------------------------------------------

#[test]
fn dns_protocol_all_variants() {
    let variants = [
        DnsProtocol::Udp,
        DnsProtocol::Tcp,
        DnsProtocol::DoH,
        DnsProtocol::DoT,
    ];
    assert_eq!(variants.len(), 4);
}

// ---------------------------------------------------------------------------
// DnssecStatus
// ---------------------------------------------------------------------------

#[test]
fn dnssec_status_all_variants() {
    let variants = [
        DnssecStatus::Secure,
        DnssecStatus::Insecure,
        DnssecStatus::Bogus,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// DomainCategory
// ---------------------------------------------------------------------------

#[test]
fn domain_category_all_variants() {
    let variants = [
        DomainCategory::Normal,
        DomainCategory::Advertising,
        DomainCategory::Tracking,
        DomainCategory::Malware,
        DomainCategory::Phishing,
        DomainCategory::Social,
        DomainCategory::Cdn,
    ];
    assert_eq!(variants.len(), 7);
}

// ---------------------------------------------------------------------------
// FirewallDirection, Action, Profile
// ---------------------------------------------------------------------------

#[test]
fn firewall_direction_all_variants() {
    let variants = [FirewallDirection::Inbound, FirewallDirection::Outbound];
    assert_eq!(variants.len(), 2);
}

#[test]
fn firewall_action_all_variants() {
    let variants = [FirewallAction::Allow, FirewallAction::Block, FirewallAction::Log];
    assert_eq!(variants.len(), 3);
}

#[test]
fn firewall_profile_all_variants() {
    let variants = [
        FirewallProfile::Domain,
        FirewallProfile::Private,
        FirewallProfile::Public,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// FirewallRule construction
// ---------------------------------------------------------------------------

#[test]
fn firewall_rule_construction() {
    let rule = FirewallRule {
        name: "Allow SSH".into(),
        enabled: true,
        direction: FirewallDirection::Inbound,
        action: FirewallAction::Allow,
        protocol: Some("TCP".into()),
        local_port: Some("22".into()),
        remote_port: None,
        local_address: None,
        remote_address: None,
        program: None,
        service: None,
        profile: FirewallProfile::Private,
        description: Some("Allow incoming SSH".into()),
        hit_count: 100,
        last_hit: Some("2026-02-12T10:00:00Z".into()),
    };
    assert_eq!(rule.name, "Allow SSH");
    assert!(rule.enabled);
    assert_eq!(rule.action, FirewallAction::Allow);
}

// ---------------------------------------------------------------------------
// AdapterType, WifiSecurity
// ---------------------------------------------------------------------------

#[test]
fn adapter_type_all_variants() {
    let variants = [
        AdapterType::Ethernet,
        AdapterType::Wifi,
        AdapterType::Cellular,
        AdapterType::Loopback,
        AdapterType::Vpn,
        AdapterType::Bridge,
        AdapterType::Virtual,
        AdapterType::Tunnel,
    ];
    assert_eq!(variants.len(), 8);
}

#[test]
fn wifi_security_all_variants() {
    let variants = [
        WifiSecurity::Open,
        WifiSecurity::Wep,
        WifiSecurity::WpaPersonal,
        WifiSecurity::WpaEnterprise,
        WifiSecurity::Wpa2Personal,
        WifiSecurity::Wpa3Personal,
    ];
    assert_eq!(variants.len(), 6);
}

// ---------------------------------------------------------------------------
// AppProtocol
// ---------------------------------------------------------------------------

#[test]
fn app_protocol_all_variants() {
    let variants = [
        AppProtocol::Http,
        AppProtocol::Https,
        AppProtocol::Ftp,
        AppProtocol::Ssh,
        AppProtocol::Smtp,
        AppProtocol::Pop3,
        AppProtocol::Imap,
        AppProtocol::Dns,
        AppProtocol::Dhcp,
        AppProtocol::Ntp,
        AppProtocol::Snmp,
        AppProtocol::Rdp,
        AppProtocol::Vnc,
        AppProtocol::Mqtt,
        AppProtocol::WebSocket,
        AppProtocol::Grpc,
        AppProtocol::Quic,
        AppProtocol::Other,
    ];
    assert_eq!(variants.len(), 18);
}

// ---------------------------------------------------------------------------
// QosPriority
// ---------------------------------------------------------------------------

#[test]
fn qos_priority_all_variants() {
    let variants = [
        QosPriority::Critical,
        QosPriority::High,
        QosPriority::Medium,
        QosPriority::Low,
        QosPriority::Background,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// NodeType
// ---------------------------------------------------------------------------

#[test]
fn node_type_all_variants() {
    let variants = [
        NodeType::Router,
        NodeType::Switch,
        NodeType::Host,
        NodeType::Firewall,
        NodeType::Unknown,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// CaptureFormat
// ---------------------------------------------------------------------------

#[test]
fn capture_format_all_variants() {
    let variants = [CaptureFormat::Pcap, CaptureFormat::PcapNg];
    assert_eq!(variants.len(), 2);
}

// ---------------------------------------------------------------------------
// NetworkOverview construction
// ---------------------------------------------------------------------------

#[test]
fn network_overview_construction() {
    let overview = NetworkOverview {
        active_connections: 42,
        total_bandwidth_in_bps: 1_000_000,
        total_bandwidth_out_bps: 500_000,
        dns_queries_per_sec: 10,
        blocked_connections: 5,
        interface_count: 3,
        vpn_active: false,
    };
    assert_eq!(overview.active_connections, 42);
    assert!(!overview.vpn_active);
}

// ---------------------------------------------------------------------------
// Serde roundtrips
// ---------------------------------------------------------------------------

#[test]
fn connection_info_serde_roundtrip() {
    let conn = ConnectionInfo {
        pid: 1234,
        process_name: "firefox".into(),
        protocol: NetworkProtocol::Tcp,
        local_address: "127.0.0.1".into(),
        local_port: 54321,
        remote_address: "93.184.216.34".into(),
        remote_port: 443,
        state: ConnectionState::Established,
        bytes_sent: 1024,
        bytes_received: 2048,
        bytes_sent_rate: 100,
        bytes_received_rate: 200,
        packets_sent: 10,
        packets_received: 20,
        latency_ms: Some(15.0),
        jitter_ms: Some(2.0),
        packet_loss_percent: Some(0.1),
        tls_version: Some(TlsVersion::Tls13),
        tls_cipher: Some("TLS_AES_256_GCM_SHA384".into()),
        sni_hostname: Some("example.com".into()),
        certificate_subject: None,
        certificate_issuer: None,
        certificate_expiry: None,
        geo_info: None,
        dns_name: Some("example.com".into()),
        app_protocol: Some("HTTPS".into()),
        socket_options: None,
        send_buffer_bytes: None,
        recv_buffer_bytes: None,
        retransmits: None,
        rtt_ms: None,
        congestion_window: None,
        mss: None,
        window_size: None,
        established_time: None,
    };
    let json = serde_json::to_string(&conn).unwrap();
    let back: ConnectionInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.pid, 1234);
    assert_eq!(back.tls_version, Some(TlsVersion::Tls13));
}

#[test]
fn dns_query_entry_serde_roundtrip() {
    let entry = DnsQueryEntry {
        timestamp: "2026-02-12T10:00:00Z".into(),
        domain: "example.com".into(),
        query_type: DnsQueryType::A,
        server: "8.8.8.8".into(),
        protocol: DnsProtocol::Udp,
        response_time_ms: 5.0,
        response: Some(DnsResponse {
            address: Some("93.184.216.34".into()),
            ttl_secs: 300,
            response_code: "NOERROR".into(),
            authoritative: false,
        }),
        pid: Some(1234),
        process_name: Some("firefox".into()),
        dnssec_status: Some(DnssecStatus::Secure),
        blocked: false,
        category: Some(DomainCategory::Normal),
        cached: false,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: DnsQueryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.domain, "example.com");
    assert_eq!(back.query_type, DnsQueryType::A);
}
