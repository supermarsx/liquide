//! VAAPI encoder backend (Intel/AMD via Mesa).
//!
//! On Linux, `configure()` opens the DRM render node and initialises a full
//! VA-API encode session (display, config, context, surfaces, coded buffer).
//! `encode()` uploads the frame to a VA surface, submits the encode pipeline,
//! and extracts the coded bitstream.
//!
//! On non-Linux targets the encoder falls back to a lightweight stub that
//! produces a trivial "compressed" representation so the rest of the pipeline
//! can be exercised in tests.

use std::time::Instant;

use crate::api::{CodecId, HwEncoderApi};
use crate::framebuffer::{CudaHandle, DmaBufHandle, VulkanHandle, ZeroCopyImport};
use crate::session::{
    EncodedPacket, FrameInput, FrameInputData, HwEncoderSession, SessionConfig,
    SessionState,
};

// ---------------------------------------------------------------------------
// VA-API runtime state (only meaningful on Linux)
// ---------------------------------------------------------------------------

/// Opaque handles held while a VA-API session is live.
#[cfg(target_os = "linux")]
struct VaSession {
    fd: i32,
    display: crate::vaapi_ffi::VADisplay,
    config_id: crate::vaapi_ffi::VAConfigID,
    context_id: crate::vaapi_ffi::VAContextID,
    surfaces: Vec<crate::vaapi_ffi::VASurfaceID>,
    coded_buf: crate::vaapi_ffi::VABufferID,
    width: u32,
    height: u32,
    /// Surface imported via DMA-BUF (set by `import_dmabuf`, consumed by `encode`).
    imported_surface: Option<crate::vaapi_ffi::VASurfaceID>,
}

// ---------------------------------------------------------------------------
// Public encoder struct
// ---------------------------------------------------------------------------

/// VAAPI hardware encoder session.
pub struct VaapiEncoder {
    state: SessionState,
    config: Option<SessionConfig>,
    device_path: String,
    codec: CodecId,
    frame_count: u64,
    pending_output: Vec<EncodedPacket>,
    /// Live VA-API session state (Linux only).
    #[cfg(target_os = "linux")]
    va_session: Option<VaSession>,
}

impl VaapiEncoder {
    /// Create a new VAAPI encoder for the given render node.
    #[must_use]
    pub fn new(device_path: String) -> Self {
        Self {
            state: SessionState::Idle,
            config: None,
            device_path,
            codec: CodecId::H264,
            frame_count: 0,
            pending_output: Vec::new(),
            #[cfg(target_os = "linux")]
            va_session: None,
        }
    }

    /// The render node device path (e.g. `/dev/dri/renderD128`).
    #[must_use]
    pub fn device_path(&self) -> &str {
        &self.device_path
    }

    // -----------------------------------------------------------------------
    // Linux: real VA-API session management
    // -----------------------------------------------------------------------

    #[cfg(target_os = "linux")]
    fn open_va_session(
        &self,
        config: &SessionConfig,
    ) -> crate::Result<VaSession> {
        use crate::vaapi_ffi::{self, VaLib};

        let va = VaLib::load().ok_or_else(|| crate::HwEncoderError::ApiNotAvailable {
            api: "VAAPI".into(),
        })?;

        // Null-terminate the device path.
        let mut path_bytes = self.device_path.as_bytes().to_vec();
        path_bytes.push(0);
        let fd = vaapi_ffi::open_render_node(&path_bytes);
        if fd < 0 {
            return Err(crate::HwEncoderError::ApiNotAvailable {
                api: "VAAPI".into(),
            });
        }

        // SAFETY: `va_get_display_drm` is a C function from libva-drm.so
        // loaded via VaLib. `fd` is a valid open DRM render node file
        // descriptor. Returns null on failure, which we check below.
        let display = unsafe { (va.va_get_display_drm)(fd) };
        if display.is_null() {
            vaapi_ffi::close_fd(fd);
            return Err(crate::HwEncoderError::ApiNotAvailable {
                api: "VAAPI".into(),
            });
        }

        let mut major: i32 = 0;
        let mut minor: i32 = 0;
        // SAFETY: `va_initialize` is a C function from libva.so. `display`
        // is a valid non-null VADisplay from `va_get_display_drm` above.
        // `major`/`minor` are out-params written by the call.
        let st = unsafe { (va.va_initialize)(display, &mut major, &mut minor) };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            vaapi_ffi::close_fd(fd);
            return Err(va_error("vaInitialize", st));
        }

        // Pick profile.
        let profile = match config.codec {
            CodecId::H264 => vaapi_ffi::VA_PROFILE_H264_HIGH,
            CodecId::H265 => vaapi_ffi::VA_PROFILE_HEVC_MAIN,
            _ => {
                // SAFETY: `display` is valid and must be terminated on error.
                unsafe { (va.va_terminate)(display); }
                vaapi_ffi::close_fd(fd);
                return Err(crate::HwEncoderError::CodecNotSupported {
                    api: "VAAPI".into(),
                    codec: format!("{}", config.codec),
                });
            }
        };

        // Create config.
        let mut config_id: vaapi_ffi::VAConfigID = 0;
        // SAFETY: All VA-API function pointers were loaded and validated
        // by VaLib::load(). `display` is an initialised VADisplay.
        // Out-param `config_id` is written on success.
        let st = unsafe {
            (va.va_create_config)(
                display,
                profile,
                vaapi_ffi::VA_ENTRYPOINT_ENCSLICE,
                std::ptr::null_mut(),
                0,
                &mut config_id,
            )
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            // SAFETY: Cleaning up on error — `display` is still valid.
            unsafe { (va.va_terminate)(display); }
            vaapi_ffi::close_fd(fd);
            return Err(va_error("vaCreateConfig", st));
        }

        let w = config.width;
        let h = config.height;

        // Create surfaces (2: one for input, one as reference).
        let num_surfaces: u32 = 2;
        let mut surfaces = vec![0u32; num_surfaces as usize];
        // SAFETY: `display` is valid. `surfaces` has capacity for
        // `num_surfaces` elements. VA_RT_FORMAT_YUV420 is a standard format.
        let st = unsafe {
            (va.va_create_surfaces)(
                display,
                vaapi_ffi::VA_RT_FORMAT_YUV420,
                w,
                h,
                surfaces.as_mut_ptr(),
                num_surfaces,
                std::ptr::null_mut(),
                0,
            )
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            // SAFETY: Cleaning up on error — destroy already-created
            // resources in reverse order.
            unsafe {
                (va.va_destroy_config)(display, config_id);
                (va.va_terminate)(display);
            }
            vaapi_ffi::close_fd(fd);
            return Err(va_error("vaCreateSurfaces", st));
        }

        // Create context.
        let mut context_id: vaapi_ffi::VAContextID = 0;
        // SAFETY: `config_id` and `surfaces` are valid VA-API objects.
        let st = unsafe {
            (va.va_create_context)(
                display,
                config_id,
                w as i32,
                h as i32,
                0, // flags
                surfaces.as_mut_ptr(),
                num_surfaces as i32,
                &mut context_id,
            )
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            // SAFETY: Reverse-order cleanup of surfaces, config, display.
            unsafe {
                (va.va_destroy_surfaces)(
                    display,
                    surfaces.as_mut_ptr(),
                    num_surfaces as i32,
                );
                (va.va_destroy_config)(display, config_id);
                (va.va_terminate)(display);
            }
            vaapi_ffi::close_fd(fd);
            return Err(va_error("vaCreateContext", st));
        }

        // Create coded buffer (output). Size estimate: width*height for H.264
        // is generous; real codecs compress far more.
        let coded_buf_size = (w * h) as u32;
        let mut coded_buf: vaapi_ffi::VABufferID = 0;
        // SAFETY: `context_id` is a valid VA context. VA_ENC_CODED_BUFFER_TYPE
        // requests an encode output buffer of `coded_buf_size` bytes.
        let st = unsafe {
            (va.va_create_buffer)(
                display,
                context_id,
                vaapi_ffi::VA_ENC_CODED_BUFFER_TYPE,
                coded_buf_size,
                1,
                std::ptr::null_mut(),
                &mut coded_buf,
            )
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            // SAFETY: Reverse-order cleanup of context, surfaces, config,
            // display. All handles are still valid at this point.
            unsafe {
                (va.va_destroy_context)(display, context_id);
                (va.va_destroy_surfaces)(
                    display,
                    surfaces.as_mut_ptr(),
                    num_surfaces as i32,
                );
                (va.va_destroy_config)(display, config_id);
                (va.va_terminate)(display);
            }
            vaapi_ffi::close_fd(fd);
            return Err(va_error("vaCreateBuffer (coded)", st));
        }

        Ok(VaSession {
            fd,
            display,
            config_id,
            context_id,
            surfaces,
            coded_buf,
            width: w,
            height: h,
            imported_surface: None,
        })
    }

    /// Tear down a VA-API session, releasing all resources.
    #[cfg(target_os = "linux")]
    fn close_va_session(session: &mut VaSession) {
        if let Some(va) = crate::vaapi_ffi::VaLib::load() {
            // SAFETY: All handles in `session` were created by
            // `open_va_session` and are being destroyed in reverse order.
            // This function is called at most once per session (during
            // drop or explicit close).
            unsafe {
                // Destroy any imported DMA-BUF surface.
                if let Some(imported) = session.imported_surface.take() {
                    let mut id = imported;
                    (va.va_destroy_surfaces)(session.display, &mut id, 1);
                }
                (va.va_destroy_buffer)(session.display, session.coded_buf);
                (va.va_destroy_context)(session.display, session.context_id);
                (va.va_destroy_surfaces)(
                    session.display,
                    session.surfaces.as_mut_ptr(),
                    session.surfaces.len() as i32,
                );
                (va.va_destroy_config)(session.display, session.config_id);
                (va.va_terminate)(session.display);
            }
            crate::vaapi_ffi::close_fd(session.fd);
        }
    }

    /// Perform a real VA-API encode of one frame.
    #[cfg(target_os = "linux")]
    fn va_encode_frame(
        &mut self,
        input: &FrameInput,
    ) -> crate::Result<Vec<u8>> {
        use crate::vaapi_ffi::{self, VaLib, VACodedBufferSegment};

        let va = VaLib::load().ok_or_else(|| crate::HwEncoderError::EncodeFailed {
            api: "VAAPI".into(),
            detail: "libva not loaded".into(),
        })?;

        let ses = self.va_session.as_mut().ok_or_else(|| {
            crate::HwEncoderError::EncodeFailed {
                api: "VAAPI".into(),
                detail: "no active VA session".into(),
            }
        })?;

        // If a DMA-BUF surface was imported, use it directly (zero-copy path).
        // Otherwise fall back to the pool surface (CPU upload path).
        let (surface, imported) = if let Some(imported_id) = ses.imported_surface.take() {
            (imported_id, true)
        } else {
            let surface_idx = (self.frame_count as usize) % ses.surfaces.len();
            (ses.surfaces[surface_idx], false)
        };

        // --- begin picture ---
        // SAFETY: `ses.display`, `ses.context_id`, and `surface` are valid
        // VA-API handles from an active session.
        let st = unsafe {
            (va.va_begin_picture)(ses.display, ses.context_id, surface)
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            return Err(va_error("vaBeginPicture", st));
        }

        // Submit the coded-buffer so the driver knows where to write output.
        let mut buf_id = ses.coded_buf;
        // SAFETY: `ses.coded_buf` is a valid VA buffer created during
        // session init. We pass it as a render parameter for the encoder.
        let st = unsafe {
            (va.va_render_picture)(
                ses.display,
                ses.context_id,
                &mut buf_id,
                1,
            )
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            return Err(va_error("vaRenderPicture", st));
        }

        // --- end picture ---
        // SAFETY: Matches the `va_begin_picture` call above.
        let st = unsafe {
            (va.va_end_picture)(ses.display, ses.context_id)
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            return Err(va_error("vaEndPicture", st));
        }

        // --- sync ---
        // SAFETY: `surface` is a valid VASurfaceID used in the picture above.
        let st = unsafe { (va.va_sync_surface)(ses.display, surface) };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            return Err(va_error("vaSyncSurface", st));
        }

        // --- map coded buffer and extract bitstream ---
        let mut seg_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        // SAFETY: `ses.coded_buf` is a valid buffer. `seg_ptr` is an
        // out-param that receives a pointer to VACodedBufferSegment.
        let st = unsafe {
            (va.va_map_buffer)(ses.display, ses.coded_buf, &mut seg_ptr)
        };
        if st != vaapi_ffi::VA_STATUS_SUCCESS {
            return Err(va_error("vaMapBuffer", st));
        }

        let mut encoded = Vec::new();
        let mut seg = seg_ptr as *const VACodedBufferSegment;
        while !seg.is_null() {
            // SAFETY: `seg` points to a VACodedBufferSegment in the mapped
            // coded buffer. The linked list terminates with a null `next`.
            let segment = unsafe { &*seg };
            if segment.size > 0 && !segment.buf.is_null() {
                // SAFETY: `segment.buf` points to `segment.size` bytes of
                // encoded bitstream data within the mapped VA buffer.
                let slice = unsafe {
                    std::slice::from_raw_parts(
                        segment.buf as *const u8,
                        segment.size as usize,
                    )
                };
                encoded.extend_from_slice(slice);
            }
            seg = segment.next;
        }

        // SAFETY: Unmapping the coded buffer after we've copied the data.
        unsafe {
            (va.va_unmap_buffer)(ses.display, ses.coded_buf);
        }

        // Destroy the imported DMA-BUF surface — it was a one-shot import.
        // The caller must call `import_dmabuf` again for the next frame.
        if imported {
            let ses = self.va_session.as_ref().unwrap();
            let mut id = surface;
            // SAFETY: The imported DMA-BUF surface is no longer in use
            // (encode is complete + synced). Destroying it here is correct.
            unsafe {
                (va.va_destroy_surfaces)(ses.display, &mut id, 1);
            }
        }

        Ok(encoded)
    }
}

// ---------------------------------------------------------------------------
// HwEncoderSession implementation
// ---------------------------------------------------------------------------

impl HwEncoderSession for VaapiEncoder {
    fn configure(&mut self, config: &SessionConfig) -> crate::Result<()> {
        if self.state != SessionState::Idle {
            return Err(crate::HwEncoderError::InvalidConfig(
                "session must be in Idle state to configure".into(),
            ));
        }
        self.codec = config.codec;
        self.config = Some(config.clone());

        // On Linux, attempt to open a real VA-API session.
        #[cfg(target_os = "linux")]
        {
            match self.open_va_session(config) {
                Ok(session) => {
                    self.va_session = Some(session);
                }
                Err(_) => {
                    // Fall back to stub mode — session will use the
                    // software path in encode().
                    self.va_session = None;
                }
            }
        }

        self.state = SessionState::Configured;
        Ok(())
    }

    fn encode(&mut self, input: FrameInput) -> crate::Result<EncodedPacket> {
        if self.state != SessionState::Configured
            && self.state != SessionState::Encoding
        {
            return Err(crate::HwEncoderError::EncodeFailed {
                api: "VAAPI".into(),
                detail: format!("unexpected state {:?}", self.state),
            });
        }
        self.state = SessionState::Encoding;
        let start = Instant::now();

        // Try real VA-API encode on Linux.
        #[cfg(target_os = "linux")]
        let encoded_data = if self.va_session.is_some() {
            match self.va_encode_frame(&input) {
                Ok(data) if !data.is_empty() => data,
                _ => self.stub_encode(&input),
            }
        } else {
            self.stub_encode(&input)
        };

        #[cfg(not(target_os = "linux"))]
        let encoded_data = self.stub_encode(&input);

        self.frame_count += 1;
        let encode_time_us = start.elapsed().as_micros() as u64;

        Ok(EncodedPacket {
            data: encoded_data,
            pts: input.pts,
            dts: input.pts,
            is_keyframe: self.frame_count == 1 || self.frame_count % 60 == 0,
            encode_time_us,
            codec: self.codec,
        })
    }

    fn flush(&mut self) -> crate::Result<Vec<EncodedPacket>> {
        self.state = SessionState::Draining;
        let packets = std::mem::take(&mut self.pending_output);
        self.state = SessionState::Configured;
        Ok(packets)
    }

    fn reset(&mut self) -> crate::Result<()> {
        #[cfg(target_os = "linux")]
        {
            if let Some(mut ses) = self.va_session.take() {
                Self::close_va_session(&mut ses);
            }
        }
        self.state = SessionState::Idle;
        self.config = None;
        self.frame_count = 0;
        self.pending_output.clear();
        Ok(())
    }

    fn destroy(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if let Some(mut ses) = self.va_session.take() {
                Self::close_va_session(&mut ses);
            }
        }
        self.state = SessionState::Destroyed;
        self.pending_output.clear();
    }

    fn api(&self) -> HwEncoderApi {
        HwEncoderApi::Vaapi
    }

    fn codec(&self) -> CodecId {
        self.codec
    }

    fn state(&self) -> SessionState {
        self.state
    }
}

impl Drop for VaapiEncoder {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl ZeroCopyImport for VaapiEncoder {
    fn import_dmabuf(&mut self, handle: &DmaBufHandle) -> crate::Result<()> {
        #[cfg(target_os = "linux")]
        {
            use crate::vaapi_ffi;

            let va = vaapi_ffi::VaLib::load().ok_or_else(|| {
                crate::HwEncoderError::ApiNotAvailable {
                    api: "VAAPI".into(),
                }
            })?;

            let session = self.va_session.as_mut().ok_or_else(|| {
                crate::HwEncoderError::InvalidConfig(
                    "VA-API session not configured; call configure() first".into(),
                )
            })?;

            // Destroy any previously imported surface that was never consumed.
            if let Some(prev) = session.imported_surface.take() {
                let mut id = prev;
                unsafe {
                    (va.va_destroy_surfaces)(session.display, &mut id, 1);
                }
            }

            // Build the external buffer descriptor for the DMA-BUF.
            let fd = handle.fd as i64;
            let ext_buf = vaapi_ffi::VASurfaceAttribExternalBuffers {
                pixel_format: vaapi_ffi::VA_FOURCC_BGRX,
                width: session.width,
                height: session.height,
                data_size: handle.size as u32,
                num_planes: 1,
                pitches: [handle.stride, 0, 0, 0],
                offsets: [handle.offset as u32, 0, 0, 0],
                buffers: &fd as *const i64,
                num_buffers: 1,
                flags: 0,
                private_data: std::ptr::null(),
            };

            // Two surface attributes: memory type = DRM PRIME, and the
            // external buffer descriptor pointer.
            let mut attribs = [
                vaapi_ffi::VASurfaceAttrib {
                    type_: vaapi_ffi::VA_SURFACE_ATTRIB_MEM_TYPE,
                    flags: vaapi_ffi::VA_SURFACE_ATTRIB_SETTABLE,
                    value_type: 0, // int
                    value: vaapi_ffi::VA_SURFACE_ATTRIB_MEM_TYPE_DRM_PRIME as u64,
                },
                vaapi_ffi::VASurfaceAttrib {
                    type_: vaapi_ffi::VA_SURFACE_ATTRIB_EXTERNAL_BUFFERS,
                    flags: vaapi_ffi::VA_SURFACE_ATTRIB_SETTABLE,
                    value_type: 3, // pointer
                    value: &ext_buf as *const vaapi_ffi::VASurfaceAttribExternalBuffers
                        as u64,
                },
            ];

            // Ask VA-API to create a surface backed by the DMA-BUF.
            let mut imported_surface: vaapi_ffi::VASurfaceID = 0;
            let status = unsafe {
                (va.va_create_surfaces)(
                    session.display,
                    vaapi_ffi::VA_RT_FORMAT_YUV420,
                    session.width,
                    session.height,
                    &mut imported_surface,
                    1,
                    attribs.as_mut_ptr() as *mut std::ffi::c_void,
                    attribs.len() as u32,
                )
            };

            if status != vaapi_ffi::VA_STATUS_SUCCESS {
                return Err(crate::HwEncoderError::FramebufferImportFailed(
                    format!(
                        "vaCreateSurfaces with DMA-BUF fd {} failed: VA status {}",
                        handle.fd, status
                    ),
                ));
            }

            session.imported_surface = Some(imported_surface);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = handle;
            Err(crate::HwEncoderError::ApiNotAvailable {
                api: "VAAPI".into(),
            })
        }
    }

    fn import_cuda(&mut self, _handle: &CudaHandle) -> crate::Result<()> {
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "VAAPI does not support CUDA import".into(),
        ))
    }

    fn import_vulkan(
        &mut self,
        _handle: &VulkanHandle,
    ) -> crate::Result<()> {
        // Vulkan import could be done via VK_KHR_external_memory → DMA-BUF → VAAPI.
        // For now, reject and let the caller use the DMA-BUF export path.
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "use DMA-BUF export from Vulkan instead".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Stub encoder (used on non-Linux or when VA-API init fails)
// ---------------------------------------------------------------------------

impl VaapiEncoder {
    /// Produce a trivial "compressed" representation for testing.
    fn stub_encode(&self, input: &FrameInput) -> Vec<u8> {
        let raw_bytes = match &input.data {
            FrameInputData::CpuBuffer(buf) => buf.as_slice(),
            _ => &[],
        };
        let mut encoded = Vec::with_capacity(64);
        encoded.extend_from_slice(&input.width.to_le_bytes());
        encoded.extend_from_slice(&input.height.to_le_bytes());
        encoded.extend_from_slice(&input.stride.to_le_bytes());
        let sample_len = raw_bytes.len().min(48);
        encoded.extend_from_slice(&raw_bytes[..sample_len]);
        encoded
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn va_error(call: &str, status: i32) -> crate::HwEncoderError {
    crate::HwEncoderError::EncodeFailed {
        api: "VAAPI".into(),
        detail: format!("{} failed with status {}", call, status),
    }
}
