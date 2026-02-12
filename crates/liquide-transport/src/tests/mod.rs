mod abr_tests;
mod backoff_tests;
mod bridge_tests;
mod codec_tests;
mod congestion_tests;
mod connection_tests;
mod fec_tests;
mod hybrid_tests;
mod listener_tests;
mod loss_tests;
mod mtu_tests;
mod negotiate_tests;
mod pool_tests;
mod priority_tests;
mod sendbuf_tests;
mod stats_tests;
mod tcp_tests;
mod test_helpers;
mod udp_tests;

#[cfg(feature = "tls")]
mod tls_tests;

#[cfg(feature = "quic")]
mod quic_tests;

#[cfg(feature = "websocket")]
mod ws_tests;
