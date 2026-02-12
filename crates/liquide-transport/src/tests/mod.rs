mod backoff_tests;
mod codec_tests;
mod connection_tests;
mod listener_tests;
mod pool_tests;
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
