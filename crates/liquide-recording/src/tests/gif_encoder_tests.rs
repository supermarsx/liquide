use crate::gif_encoder::GifEncoder;

#[test]
fn test_gif_encoder_empty() {
    let enc = GifEncoder::new(4, 4, 30);
    assert_eq!(enc.frame_count(), 0);
    let gif = enc.finish();
    // Should at least have header + GCT + NETSCAPE ext + trailer
    assert!(gif.len() > 10);
    assert_eq!(&gif[..6], b"GIF89a");
    assert_eq!(gif[gif.len() - 1], 0x3B);
}

#[test]
fn test_gif_encoder_single_frame() {
    let mut enc = GifEncoder::new(2, 2, 10);
    let frame = vec![
        255, 0, 0, 255,   // red
        0, 255, 0, 255,   // green
        0, 0, 255, 255,   // blue
        255, 255, 0, 255,  // yellow
    ];
    enc.add_frame(&frame);
    assert_eq!(enc.frame_count(), 1);

    let gif = enc.finish();
    assert_eq!(&gif[..6], b"GIF89a");
    // Width and height in logical screen descriptor (bytes 6-9)
    assert_eq!(u16::from_le_bytes([gif[6], gif[7]]), 2);
    assert_eq!(u16::from_le_bytes([gif[8], gif[9]]), 2);
    assert_eq!(gif[gif.len() - 1], 0x3B);
}

#[test]
fn test_gif_encoder_multiple_frames() {
    let mut enc = GifEncoder::new(4, 4, 15);
    let frame_red = vec![255u8, 0, 0, 255].repeat(16);
    let frame_blue = vec![0u8, 0, 255, 255].repeat(16);

    enc.add_frame(&frame_red);
    enc.add_frame(&frame_blue);
    enc.add_frame(&frame_red);
    assert_eq!(enc.frame_count(), 3);

    let gif = enc.finish();
    assert_eq!(&gif[..6], b"GIF89a");
    assert_eq!(gif[gif.len() - 1], 0x3B);
    // Should be larger than single frame
    assert!(gif.len() > 800);
}

#[test]
fn test_gif_encoder_grayscale_frame() {
    let mut enc = GifEncoder::new(4, 4, 30);
    // All gray pixels
    let mut frame = Vec::with_capacity(4 * 4 * 4);
    for i in 0..16 {
        let v = (i * 16) as u8;
        frame.extend_from_slice(&[v, v, v, 255]);
    }
    enc.add_frame(&frame);
    let gif = enc.finish();
    assert_eq!(&gif[..6], b"GIF89a");
}

#[test]
fn test_gif_encoder_undersized_frame_ignored() {
    let mut enc = GifEncoder::new(8, 8, 30);
    // Only 4 bytes instead of 8*8*4=256
    enc.add_frame(&[0, 0, 0, 255]);
    assert_eq!(enc.frame_count(), 0); // frame should be skipped
}

#[test]
fn test_gif_encoder_delay_computation() {
    let enc = GifEncoder::new(4, 4, 10);
    assert_eq!(enc.delay_cs(), 10); // 100/10 = 10 cs

    let enc2 = GifEncoder::new(4, 4, 50);
    assert_eq!(enc2.delay_cs(), 2); // 100/50 = 2 cs (minimum)

    let enc3 = GifEncoder::new(4, 4, 0);
    assert_eq!(enc3.delay_cs(), 10); // fallback
}

#[test]
fn test_gif_encoder_current_size_grows() {
    let mut enc = GifEncoder::new(4, 4, 30);
    let before = enc.current_size();

    let frame = vec![128u8; 4 * 4 * 4];
    enc.add_frame(&frame);
    let after = enc.current_size();

    assert!(after > before);
}

#[test]
fn test_gif_encoder_display() {
    let enc = GifEncoder::new(320, 240, 30);
    let s = format!("{enc}");
    assert!(s.contains("320x240"));
    assert!(s.contains("frames=0"));
}

#[test]
fn test_gif_encoder_large_frame() {
    // 64x64 frame — tests that LZW handles non-trivial data
    let mut enc = GifEncoder::new(64, 64, 30);
    let mut frame = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64u32 {
        for x in 0..64u32 {
            frame.push((x * 4) as u8);   // R
            frame.push((y * 4) as u8);   // G
            frame.push(128);              // B
            frame.push(255);              // A
        }
    }
    enc.add_frame(&frame);
    let gif = enc.finish();
    assert_eq!(&gif[..6], b"GIF89a");
    assert_eq!(gif[gif.len() - 1], 0x3B);
    // LZW compressed data should be smaller than raw (64*64 = 4096 indices)
    assert!(gif.len() < 64 * 64 * 4);
}
