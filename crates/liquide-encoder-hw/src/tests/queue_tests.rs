use crate::api::HwEncoderApi;
use crate::queue::EncoderQueueManager;

#[test]
fn register_and_allocate() {
    let mut mgr = EncoderQueueManager::new();
    mgr.register_gpu(0, HwEncoderApi::Vaapi, 4, 1024);
    assert_eq!(mgr.total_active_sessions(), 0);

    mgr.allocate_session(0, 128).unwrap();
    assert_eq!(mgr.total_active_sessions(), 1);
}

#[test]
fn session_limit_enforced() {
    let mut mgr = EncoderQueueManager::new();
    mgr.register_gpu(0, HwEncoderApi::Nvenc, 2, 4096);
    mgr.allocate_session(0, 64).unwrap();
    mgr.allocate_session(0, 64).unwrap();
    assert!(mgr.allocate_session(0, 64).is_err());
}

#[test]
fn vram_limit_enforced() {
    let mut mgr = EncoderQueueManager::new();
    mgr.register_gpu(0, HwEncoderApi::Amf, 10, 256);
    assert!(mgr.allocate_session(0, 300).is_err());
}

#[test]
fn best_gpu_selects_least_loaded() {
    let mut mgr = EncoderQueueManager::new();
    mgr.register_gpu(0, HwEncoderApi::Vaapi, 4, 1024);
    mgr.register_gpu(1, HwEncoderApi::Nvenc, 4, 1024);
    mgr.allocate_session(0, 64).unwrap();
    mgr.allocate_session(0, 64).unwrap();
    assert_eq!(mgr.best_gpu(), Some(1));
}

#[test]
fn release_session() {
    let mut mgr = EncoderQueueManager::new();
    mgr.register_gpu(0, HwEncoderApi::Vaapi, 4, 1024);
    mgr.allocate_session(0, 128).unwrap();
    mgr.release_session(0, 128);
    assert_eq!(mgr.total_active_sessions(), 0);
}

#[test]
fn is_full() {
    let mut mgr = EncoderQueueManager::new();
    mgr.register_gpu(0, HwEncoderApi::V4l2, 1, 512);
    assert!(!mgr.is_full(0));
    mgr.allocate_session(0, 64).unwrap();
    assert!(mgr.is_full(0));
}
