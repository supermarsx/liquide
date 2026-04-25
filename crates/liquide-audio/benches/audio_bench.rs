use criterion::{Criterion, black_box, criterion_group, criterion_main};

use liquide_audio::buffer::AudioRingBuffer;
use liquide_audio::codec::AudioCodec;
use liquide_audio::codec::PcmCodec;
use liquide_audio::format::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};

fn bench_ring_buffer_write_read_1m_samples(c: &mut Criterion) {
    let fmt = AudioFormat::new(
        SampleFormat::F32,
        SampleRate::Hz48000,
        ChannelLayout::Stereo,
    );
    // 1M stereo f32 samples = 1_000_000 * 2 channels * 4 bytes = 8_000_000 bytes
    // Use a large ring buffer
    let chunk_size = 8_000;
    let total_bytes = 8_000_000usize;
    let chunk = vec![0u8; chunk_size];

    c.bench_function("ring_buffer_write_read_1m_samples", |b| {
        b.iter(|| {
            let mut ring = AudioRingBuffer::new(total_bytes, fmt);
            let mut written = 0;
            while written + chunk_size <= total_bytes {
                ring.write(black_box(&chunk)).unwrap();
                written += chunk_size;
            }
            let mut read_buf = vec![0u8; chunk_size];
            let mut total_read = 0;
            while total_read + chunk_size <= total_bytes {
                ring.read(black_box(&mut read_buf)).unwrap();
                total_read += chunk_size;
            }
            black_box(total_read);
        })
    });
}

fn bench_pcm_codec_encode_10000_frames(c: &mut Criterion) {
    let fmt = AudioFormat::new(
        SampleFormat::F32,
        SampleRate::Hz48000,
        ChannelLayout::Stereo,
    );
    let frame_size = fmt.frame_size();
    // 10000 frames
    let data = vec![0u8; frame_size * 10_000];

    c.bench_function("pcm_codec_encode_10000_frames", |b| {
        b.iter(|| {
            let mut codec = PcmCodec::new();
            let encoded = codec.encode(black_box(&data)).unwrap();
            black_box(encoded);
        })
    });
}

criterion_group!(
    benches,
    bench_ring_buffer_write_read_1m_samples,
    bench_pcm_codec_encode_10000_frames,
);
criterion_main!(benches);
