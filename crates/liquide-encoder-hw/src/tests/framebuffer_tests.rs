use crate::framebuffer::*;

#[test]
fn dmabuf_handle_fields() {
    let h = DmaBufHandle { fd: 42, offset: 0, stride: 7680, size: 33177600 };
    assert_eq!(h.fd, 42);
    assert_eq!(h.stride, 7680);
}

#[test]
fn cuda_handle_fields() {
    let h = CudaHandle { device_ptr: 0xDEAD_BEEF, size: 1024 };
    assert_eq!(h.device_ptr, 0xDEAD_BEEF);
}

#[test]
fn vulkan_handle_fields() {
    let h = VulkanHandle { memory: 1, offset: 0, size: 4096, image: 2 };
    assert_eq!(h.image, 2);
}
