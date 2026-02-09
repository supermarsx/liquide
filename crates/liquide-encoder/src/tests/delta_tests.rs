use crate::delta::*;

#[test]
fn xor_delta_identical() {
    let a = vec![1, 2, 3, 4];
    let delta = xor_delta(&a, &a);
    assert!(delta.iter().all(|&b| b == 0));
    assert_eq!(change_ratio(&delta), 0.0);
}

#[test]
fn xor_delta_roundtrip() {
    let prev = vec![10, 20, 30, 40, 50];
    let curr = vec![10, 25, 30, 45, 50];
    let delta = xor_delta(&curr, &prev);
    let reconstructed = xor_apply(&prev, &delta);
    assert_eq!(reconstructed, curr);
}

#[test]
fn xor_popcount_partial() {
    let delta = vec![0, 0xFF, 0, 0xFF, 0, 0, 0, 0];
    assert_eq!(xor_popcount(&delta), 2);
    assert!((change_ratio(&delta) - 0.25).abs() < f32::EPSILON);
}

#[test]
fn xor_delta_completely_different() {
    let prev = vec![0x00; 8];
    let curr = vec![0xFF; 8];
    let delta = xor_delta(&curr, &prev);
    assert_eq!(xor_popcount(&delta), 8);
    assert!((change_ratio(&delta) - 1.0).abs() < f32::EPSILON);
}
