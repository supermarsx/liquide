use crate::meter::AudioMeter;

#[test]
fn meter_silence_is_zero() {
    let meter = AudioMeter::new(0.95);
    assert_eq!(meter.peak(), 0.0);
    assert_eq!(meter.rms(), 0.0);
    assert_eq!(meter.peak_db(), -f32::INFINITY);
    assert_eq!(meter.rms_db(), -f32::INFINITY);
}

#[test]
fn meter_known_signal_peak() {
    let mut meter = AudioMeter::new(0.0); // No decay
    let samples = vec![0.5f32, -0.75, 0.25, -1.0, 0.0];
    meter.feed_samples_f32(&samples);
    assert!((meter.peak() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn meter_decay() {
    let mut meter = AudioMeter::new(0.5);
    // Feed a loud signal
    meter.feed_samples_f32(&[1.0]);
    let peak_after_loud = meter.peak();
    assert!((peak_after_loud - 1.0).abs() < f32::EPSILON);

    // Feed silence — peak should decay
    meter.feed_samples_f32(&[0.0]);
    let peak_after_silence = meter.peak();
    // Decayed peak = 1.0 * 0.5 = 0.5, new max = 0.0, so max(0.5, 0.0) = 0.5
    assert!((peak_after_silence - 0.5).abs() < f32::EPSILON);
}

#[test]
fn meter_reset() {
    let mut meter = AudioMeter::new(0.95);
    meter.feed_samples_f32(&[0.5, -0.5, 0.8]);
    assert!(meter.peak() > 0.0);
    meter.reset();
    assert_eq!(meter.peak(), 0.0);
    assert_eq!(meter.rms(), 0.0);
}
