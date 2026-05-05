#![no_main]
use bytes::BytesMut;
use libfuzzer_sys::fuzz_target;
use liquide_protocol::codec::FrameCodec;

fuzz_target!(|data: &[u8]| {
    let mut codec = FrameCodec::new();
    let mut input = BytesMut::from(data);

    while !input.is_empty() {
        let before = input.len();
        match codec.decode_frame(&mut input) {
            Ok(Some(frame)) => {
                let _ = frame.header.msg_type();
                let _ = frame.header.wire_len();
                let _ = frame.payload.len();
            }
            Ok(None) | Err(_) => break,
        }

        if input.len() == before {
            break;
        }
    }
});
