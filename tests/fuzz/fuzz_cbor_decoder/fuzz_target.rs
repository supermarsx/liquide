#![no_main]
use libfuzzer_sys::fuzz_target;
use liquide_protocol::codec::cbor_decode;
use liquide_protocol::messages::control::{
    ChannelOpenMsg, ClientHello, DisconnectMsg, LoginPrompt, LoginResponse, Ping, Pong, ServerHello,
};
use liquide_protocol::messages::video::{
    CodecSwitchMsg, KeyFrameRequestMsg, QualityHintMsg, VideoFrameAckMsg, VideoFrameDataMsg,
    VideoFrameHeaderMsg,
};

fuzz_target!(|data: &[u8]| {
    let (selector, payload) = data
        .split_first()
        .map_or((0, data), |(head, tail)| (*head, tail));

    match selector % 14 {
        0 => {
            let _ = cbor_decode::<ClientHello>(payload);
        }
        1 => {
            let _ = cbor_decode::<ServerHello>(payload);
        }
        2 => {
            let _ = cbor_decode::<Ping>(payload);
        }
        3 => {
            let _ = cbor_decode::<Pong>(payload);
        }
        4 => {
            let _ = cbor_decode::<ChannelOpenMsg>(payload);
        }
        5 => {
            let _ = cbor_decode::<LoginPrompt>(payload);
        }
        6 => {
            let _ = cbor_decode::<LoginResponse>(payload);
        }
        7 => {
            let _ = cbor_decode::<DisconnectMsg>(payload);
        }
        8 => {
            let _ = cbor_decode::<VideoFrameHeaderMsg>(payload);
        }
        9 => {
            let _ = cbor_decode::<VideoFrameDataMsg>(payload);
        }
        10 => {
            let _ = cbor_decode::<VideoFrameAckMsg>(payload);
        }
        11 => {
            let _ = cbor_decode::<QualityHintMsg>(payload);
        }
        12 => {
            let _ = cbor_decode::<CodecSwitchMsg>(payload);
        }
        _ => {
            let _ = cbor_decode::<KeyFrameRequestMsg>(payload);
        }
    }
});
