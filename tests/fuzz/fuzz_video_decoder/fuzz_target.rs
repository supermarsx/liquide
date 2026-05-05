#![no_main]
use libfuzzer_sys::fuzz_target;
use liquide_client_renderer::{NullDecoder, VideoDecoder};
use liquide_encoder_hw::api::CodecId;

fuzz_target!(|data: &[u8]| {
    for codec in [CodecId::H264, CodecId::H265, CodecId::Av1] {
        let mut decoder = NullDecoder::new(codec);
        let _ = decoder.decode(data);
        let _ = decoder.flush();
        decoder.reset();
    }
});
