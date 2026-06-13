#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::config::StandaloneConfig;
    use crate::display::{DisplayOutput, OutputInfo};
    use crate::input::InputDeviceSummary;
    use crate::launcher::{
        DevWindowBackend, StandaloneGeometryFallbackReason, StandaloneLaunchRuntimeInputs,
        StandaloneLaunchSummary, StandaloneLauncher, StandalonePresentFeedbackFallbackReason,
        StandaloneSurfaceOverride,
    };
    use crate::wayland::WaylandServerState;
    use crate::xwayland_bridge::XWaylandBridgeState;
    use liquide_drm::mode::{DrmMode, ModeFlags};
    use liquide_drm::{ConnectorId, ConnectorInfo, ConnectorStatus, ConnectorType, SubpixelOrder};
    use liquide_platform::standalone::{
        StandaloneConfig as PlatformStandaloneConfig, StandalonePlatform, StandalonePresentMode,
        StandaloneScriptHandle,
    };
    use liquide_platform::{NativeWindowHandle, PlatformEvent};
    use liquide_session::desktop::DesktopCompositor;

    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    #[cfg(target_os = "linux")]
    const TEST_DRM_EVENT_VBLANK: u32 = 0x01;
    #[cfg(target_os = "linux")]
    const TEST_DRM_EVENT_FLIP_COMPLETE: u32 = 0x02;

    #[derive(Debug, Clone)]
    struct DesktopRunOutcome {
        frame_count: u64,
        running: bool,
        script: StandaloneScriptHandle,
    }

    #[derive(Debug, Clone)]
    struct LauncherRunOutcome {
        summary: StandaloneLaunchSummary,
        running: bool,
        script: StandaloneScriptHandle,
    }

    #[cfg(target_os = "linux")]
    struct TestPipe {
        read_fd: i32,
        write_fd: i32,
    }

    #[cfg(target_os = "linux")]
    impl TestPipe {
        fn new() -> Self {
            let mut fds = [0; 2];
            let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
            assert_eq!(
                result,
                0,
                "pipe creation failed: {}",
                std::io::Error::last_os_error()
            );

            Self {
                read_fd: fds[0],
                write_fd: fds[1],
            }
        }

        fn read_fd(&self) -> i32 {
            self.read_fd
        }

        fn write_all(&self, bytes: &[u8]) {
            let mut written = 0;
            while written < bytes.len() {
                let result = unsafe {
                    libc::write(
                        self.write_fd,
                        bytes[written..].as_ptr().cast::<libc::c_void>(),
                        bytes.len() - written,
                    )
                };
                assert!(
                    result >= 0,
                    "pipe write failed: {}",
                    std::io::Error::last_os_error()
                );
                written += result as usize;
            }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for TestPipe {
        fn drop(&mut self) {
            if self.read_fd >= 0 {
                unsafe {
                    libc::close(self.read_fd);
                }
            }
            if self.write_fd >= 0 {
                unsafe {
                    libc::close(self.write_fd);
                }
            }
        }
    }

    fn scripted_window() -> NativeWindowHandle {
        NativeWindowHandle(1)
    }

    fn scripted_output(name: &str, width: u32, height: u32, refresh_hz: u32) -> OutputInfo {
        OutputInfo {
            connector_id: 1,
            name: name.to_string(),
            mode: scripted_mode(
                width,
                height,
                refresh_hz,
                ModeFlags::PREFERRED,
                &format!("{width}x{height}@{refresh_hz}"),
            ),
            physical_width_mm: 520,
            physical_height_mm: 290,
            primary: true,
        }
    }

    fn scripted_mode(
        width: u32,
        height: u32,
        refresh_hz: u32,
        flags: ModeFlags,
        name: &str,
    ) -> DrmMode {
        DrmMode {
            width,
            height,
            refresh_hz,
            clock_khz: 0,
            flags,
            name: name.to_string(),
        }
    }

    fn scripted_connector(
        connector_id: u32,
        name: &str,
        status: ConnectorStatus,
        modes: Vec<DrmMode>,
    ) -> ConnectorInfo {
        ConnectorInfo {
            id: ConnectorId(connector_id),
            connector_type: ConnectorType::DisplayPort,
            connector_type_id: connector_id,
            name: name.to_string(),
            status,
            modes,
            physical_width_mm: 520,
            physical_height_mm: 290,
            subpixel_order: SubpixelOrder::Unknown,
            encoder_id: Some(connector_id),
        }
    }

    fn wait_until<F>(label: &str, timeout: Duration, mut predicate: F)
    where
        F: FnMut() -> bool,
    {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if predicate() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }

        panic!("timed out waiting for {label}");
    }

    fn join_or_resume(handle: thread::JoinHandle<()>) {
        if let Err(error) = handle.join() {
            std::panic::resume_unwind(error);
        }
    }

    #[cfg(target_os = "linux")]
    fn build_vblank_like_record(
        event_type: u32,
        seconds: u32,
        microseconds: u32,
        sequence: u32,
        crtc_id: u32,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32_native(&mut bytes, event_type);
        push_u32_native(&mut bytes, 32);
        push_u64_native(&mut bytes, 0);
        push_u32_native(&mut bytes, seconds);
        push_u32_native(&mut bytes, microseconds);
        push_u32_native(&mut bytes, sequence);
        push_u32_native(&mut bytes, crtc_id);
        bytes
    }

    #[cfg(target_os = "linux")]
    fn push_u32_native(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }

    #[cfg(target_os = "linux")]
    fn push_u64_native(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }

    #[cfg(target_os = "linux")]
    fn queued_feedback_runtime_inputs(
        width: u32,
        height: u32,
        refresh_hz: u32,
        present_feedback_fd: i32,
    ) -> StandaloneLaunchRuntimeInputs {
        StandaloneLaunchRuntimeInputs {
            primary_output: Some(scripted_output("TEST-1", width, height, refresh_hz)),
            live_present_feedback_capable: true,
            present_feedback_fd: Some(present_feedback_fd),
            ..StandaloneLaunchRuntimeInputs::default()
        }
    }

    #[cfg(target_os = "linux")]
    fn assert_launcher_queued_feedback_summary(
        summary: &StandaloneLaunchSummary,
        width: u32,
        height: u32,
        refresh_hz: u32,
    ) {
        assert_eq!(summary.width, width);
        assert_eq!(summary.height, height);
        assert_eq!(summary.refresh_hz, refresh_hz);
        assert_eq!(summary.requested_fps_cap, 0);
        assert_eq!(summary.effective_fps_cap, 0);
        assert_eq!(summary.present_mode, StandalonePresentMode::Queued);
        assert!(summary.live_present_feedback_capable);
        assert_eq!(summary.output_name.as_deref(), Some("TEST-1"));
        assert!(summary.fallback_reason.geometry.is_none());
        assert!(summary.fallback_reason.present_feedback.is_none());
    }

    fn run_launcher_with_optional_runtime_inputs<F>(
        config: StandaloneConfig,
        runtime_inputs: Option<StandaloneLaunchRuntimeInputs>,
        controller: F,
    ) -> LauncherRunOutcome
    where
        F: FnOnce(StandaloneLaunchSummary, StandaloneScriptHandle) + Send + 'static,
    {
        let (observed_tx, observed_rx) = mpsc::sync_channel(1);
        let controller_handle = Arc::new(Mutex::new(None));
        let mut launcher = StandaloneLauncher::new(config);

        let observer = {
            let controller_handle = Arc::clone(&controller_handle);
            move |summary: StandaloneLaunchSummary, script: StandaloneScriptHandle| {
                observed_tx
                    .send((summary.clone(), script.clone()))
                    .expect("launcher observer should capture summary and script handle");

                script.push_event(PlatformEvent::WindowResized {
                    handle: scripted_window(),
                    width: summary.width,
                    height: summary.height,
                });

                let recovery_script = script.clone();
                let handle = thread::spawn(move || {
                    let controller_result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            controller(summary, script)
                        }));
                    if let Err(panic_payload) = controller_result {
                        recovery_script.push_event(PlatformEvent::Quit);
                        std::panic::resume_unwind(panic_payload);
                    }
                });

                *controller_handle
                    .lock()
                    .expect("launcher controller slot should be available") = Some(handle);
            }
        };

        match runtime_inputs {
            Some(runtime_inputs) => launcher.run_with_runtime_inputs(runtime_inputs, observer),
            None => launcher.run_with_observer(observer),
        }
        .expect("launcher should run through the synthetic standalone handoff");

        let controller_handle = controller_handle
            .lock()
            .expect("launcher controller slot should be accessible after run")
            .take()
            .expect("launcher observer should start a controller thread");
        join_or_resume(controller_handle);

        let (summary, script) = observed_rx
            .recv()
            .expect("launcher observer should publish summary and script state");
        LauncherRunOutcome {
            summary,
            running: launcher.is_running(),
            script,
        }
    }

    fn run_launcher_with_controller<F>(
        config: StandaloneConfig,
        controller: F,
    ) -> LauncherRunOutcome
    where
        F: FnOnce(StandaloneLaunchSummary, StandaloneScriptHandle) + Send + 'static,
    {
        run_launcher_with_optional_runtime_inputs(config, None, controller)
    }

    #[cfg(target_os = "linux")]
    fn run_launcher_with_runtime_inputs<F>(
        config: StandaloneConfig,
        runtime_inputs: StandaloneLaunchRuntimeInputs,
        controller: F,
    ) -> LauncherRunOutcome
    where
        F: FnOnce(StandaloneLaunchSummary, StandaloneScriptHandle) + Send + 'static,
    {
        run_launcher_with_optional_runtime_inputs(config, Some(runtime_inputs), controller)
    }

    fn run_desktop_with_controller<F>(
        width: u32,
        height: u32,
        present_mode: StandalonePresentMode,
        fps_cap: u32,
        controller: F,
    ) -> DesktopRunOutcome
    where
        F: FnOnce(StandaloneScriptHandle) + Send + 'static,
    {
        let mut platform = StandalonePlatform::new(PlatformStandaloneConfig {
            width,
            height,
            hardware_cursor: false,
            present_mode,
            drm_event_fd: None,
        })
        .expect("standalone platform should construct for desktop integration tests");
        let script = platform.script_handle();
        script.push_event(PlatformEvent::WindowResized {
            handle: scripted_window(),
            width,
            height,
        });
        let controller_handle = {
            let controller_script = script.clone();
            thread::spawn(move || controller(controller_script))
        };

        let mut desktop = DesktopCompositor::new(width, height);
        desktop.set_fps_cap(fps_cap);
        desktop.run(&mut platform);

        join_or_resume(controller_handle);

        DesktopRunOutcome {
            frame_count: desktop.frame_count(),
            running: desktop.is_running(),
            script,
        }
    }

    #[test]
    fn dev_mode_selects_host_window_backend_for_target_os() {
        // Regression for t54: `--dev-mode` must run in a host-OS window and
        // bypass DRM/KMS + evdev (which fail off a real Linux TTY — e.g.
        // "no suitable DRM device found" on Windows). The backend selection is
        // per target OS. This is host-safe: it asserts the selection only and
        // never opens a window or touches a DRM device.
        let backend = DevWindowBackend::for_target();

        #[cfg(windows)]
        assert_eq!(backend, DevWindowBackend::Win32);
        #[cfg(target_os = "linux")]
        assert_eq!(backend, DevWindowBackend::X11);
        #[cfg(target_os = "macos")]
        assert_eq!(backend, DevWindowBackend::MacOS);

        // The selected dev backend is never the DRM/standalone path.
        assert!(matches!(
            backend,
            DevWindowBackend::Win32
                | DevWindowBackend::X11
                | DevWindowBackend::Wayland
                | DevWindowBackend::MacOS
        ));

        // A dev_mode config carries the windowed flag end-to-end; the launcher
        // never requires a DRM device for it (setup_display is skipped in
        // main.rs, and run() dispatches to the host-window path).
        let config = StandaloneConfig {
            dev_mode: true,
            width: Some(1270),
            height: Some(768),
            ..StandaloneConfig::default()
        };
        assert!(config.dev_mode);
        let launcher = StandaloneLauncher::new(config);
        let runtime_inputs = StandaloneLaunchRuntimeInputs::from_launcher(&launcher);
        // Geometry override reaches the launch plan that sizes the dev window.
        let summary = runtime_inputs.launch_summary(0);
        assert_eq!(summary.width, 1270);
        assert_eq!(summary.height, 768);
    }

    #[test]
    fn test_standalone_config_default() {
        let config = StandaloneConfig::default();
        assert!(!config.dev_mode);
        assert!(config.vt_number.is_none());
        assert!(config.drm_device.is_none());
        assert_eq!(config.fps_cap, 0);
        assert_eq!(config.wayland_socket, "wayland-0");
        assert!(config.enable_xwayland);
        assert!(config.enable_wayland);
        assert!(config.width.is_none());
        assert!(config.height.is_none());
    }

    #[test]
    fn surface_override_applies_width_height_over_fallback_initial_dimensions() {
        // With no primary output, the launch plan falls back to 1920x1080.
        // An explicit width/height override (the `--width/--height` flags) must
        // win over that fallback, becoming the initial compositor/session
        // dimensions that size the dev-mode host window. An unset dimension
        // must preserve the existing fallback exactly.
        let base = StandaloneLaunchRuntimeInputs::default();
        let baseline = base.launch_summary(0);
        assert_eq!(baseline.width, 1920);
        assert_eq!(baseline.height, 1080);

        let overridden = StandaloneLaunchRuntimeInputs {
            surface_override: StandaloneSurfaceOverride {
                width: Some(1270),
                height: Some(768),
            },
            ..StandaloneLaunchRuntimeInputs::default()
        };
        let summary = overridden.launch_summary(0);
        // Override applied: these become DesktopCompositor::new(width, height).
        assert_eq!(summary.width, 1270);
        assert_eq!(summary.height, 768);
        // Everything else stays on the existing fallback path untouched.
        assert_eq!(summary.refresh_hz, 60);
        assert_eq!(summary.present_mode, StandalonePresentMode::Immediate);
        assert_eq!(
            summary.fallback_reason.geometry,
            Some(StandaloneGeometryFallbackReason::NoOutputMetadata)
        );

        // Partial override: only width set; height stays on the fallback.
        let width_only = StandaloneLaunchRuntimeInputs {
            surface_override: StandaloneSurfaceOverride {
                width: Some(1270),
                height: None,
            },
            ..StandaloneLaunchRuntimeInputs::default()
        };
        let width_only_summary = width_only.launch_summary(0);
        assert_eq!(width_only_summary.width, 1270);
        assert_eq!(width_only_summary.height, 1080);

        // Zero is treated as unset, preserving the fallback dimension.
        let zeroed = StandaloneLaunchRuntimeInputs {
            surface_override: StandaloneSurfaceOverride {
                width: Some(0),
                height: Some(0),
            },
            ..StandaloneLaunchRuntimeInputs::default()
        };
        let zeroed_summary = zeroed.launch_summary(0);
        assert_eq!(zeroed_summary.width, 1920);
        assert_eq!(zeroed_summary.height, 1080);
    }

    #[test]
    fn surface_override_threads_through_standalone_config_into_runtime_inputs() {
        // The CLI flags land in StandaloneConfig.width/height; the launcher must
        // carry them into the runtime inputs' surface override so they reach the
        // initial dimensions.
        let config = StandaloneConfig {
            width: Some(1270),
            height: Some(768),
            ..StandaloneConfig::default()
        };
        let launcher = StandaloneLauncher::new(config);
        let runtime_inputs = StandaloneLaunchRuntimeInputs::from_launcher(&launcher);
        assert_eq!(runtime_inputs.surface_override.width, Some(1270));
        assert_eq!(runtime_inputs.surface_override.height, Some(768));

        let summary = runtime_inputs.launch_summary(0);
        assert_eq!(summary.width, 1270);
        assert_eq!(summary.height, 768);
    }

    #[test]
    fn test_display_output() {
        let mut display = DisplayOutput::new();
        assert!(display.primary().is_none());
        display.add_output(OutputInfo {
            connector_id: 1,
            name: "HDMI-A-1".to_string(),
            mode: DrmMode {
                width: 1920,
                height: 1080,
                refresh_hz: 60,
                clock_khz: 148500,
                flags: ModeFlags::PREFERRED,
                name: "1920x1080@60".to_string(),
            },
            physical_width_mm: 520,
            physical_height_mm: 290,
            primary: true,
        });
        assert!(display.primary().is_some());
        assert_eq!(display.outputs().len(), 1);
    }

    #[test]
    fn display_output_builder_prefers_current_and_preferred_connector_metadata() {
        let fallback = scripted_connector(
            1,
            "HDMI-A-1",
            ConnectorStatus::Connected,
            vec![scripted_mode(
                1920,
                1080,
                60,
                ModeFlags::empty(),
                "fallback",
            )],
        );
        let preferred = scripted_connector(
            2,
            "DP-1",
            ConnectorStatus::Connected,
            vec![scripted_mode(
                2560,
                1440,
                144,
                ModeFlags::PREFERRED,
                "preferred",
            )],
        );
        let current = scripted_connector(
            3,
            "eDP-1",
            ConnectorStatus::Connected,
            vec![
                scripted_mode(2880, 1800, 120, ModeFlags::PREFERRED, "native"),
                scripted_mode(2256, 1504, 60, ModeFlags::CURRENT, "current"),
            ],
        );

        let display = DisplayOutput::from_connectors(&[fallback, preferred, current]);

        assert_eq!(display.outputs().len(), 3);
        assert_eq!(display.outputs()[1].mode.name, "preferred");
        assert_eq!(display.outputs()[2].mode.name, "current");
        assert_eq!(
            display
                .outputs()
                .iter()
                .filter(|output| output.primary)
                .count(),
            1
        );

        let primary = display
            .primary()
            .expect("builder should select a primary output when usable modes exist");
        assert_eq!(primary.name, "eDP-1");
        assert_eq!(primary.mode.width, 2256);
        assert_eq!(primary.mode.height, 1504);
        assert_eq!(primary.mode.refresh_hz, 60);
    }

    #[test]
    fn test_input_summary_empty() {
        let summary = InputDeviceSummary::default();
        assert!(!summary.has_basic_input());
    }

    #[test]
    fn test_input_summary_from_devices() {
        use liquide_libinput::{DeviceCapability, DeviceClass, DeviceInfo};
        let devices = vec![
            DeviceInfo {
                path: "/dev/input/event0".to_string(),
                name: "Keyboard".to_string(),
                device_class: DeviceClass::Keyboard,
                capabilities: DeviceCapability::KEY,
                vendor_id: 0,
                product_id: 0,
                bus_type: 0,
            },
            DeviceInfo {
                path: "/dev/input/event1".to_string(),
                name: "Mouse".to_string(),
                device_class: DeviceClass::Mouse,
                capabilities: DeviceCapability::KEY.union(DeviceCapability::REL),
                vendor_id: 0,
                product_id: 0,
                bus_type: 0,
            },
        ];
        let summary = InputDeviceSummary::from_devices(devices);
        assert!(summary.has_basic_input());
        assert_eq!(summary.keyboard_count, 1);
        assert_eq!(summary.pointer_count, 1);
    }

    #[test]
    fn test_wayland_server_state() {
        let state = WaylandServerState::new();
        assert!(!state.is_accepting());
        assert_eq!(state.client_count(), 0);
    }

    #[test]
    fn test_xwayland_bridge_state() {
        let state = XWaylandBridgeState::default();
        assert!(!state.enabled);
        assert_eq!(state.window_count, 0);
    }

    #[test]
    fn desktop_handoff_captures_frame_and_advances_ack_only_after_feedback() {
        let width = 320;
        let height = 180;

        let outcome = run_desktop_with_controller(
            width,
            height,
            StandalonePresentMode::Queued,
            0,
            move |script| {
                wait_until("first queued present", TEST_TIMEOUT, || {
                    script.present_count() >= 1 && script.last_presented_frame().is_some()
                });

                assert_eq!(script.present_count(), 1);
                assert_eq!(script.pending_present_count(), 1);
                assert_eq!(script.acknowledged_present_count(), 0);
                assert!(!script.present_ready());

                script.push_present_ack(Some(11), Some(22), Some(33));

                wait_until(
                    "desktop frame after first scripted acknowledgement",
                    TEST_TIMEOUT,
                    || script.present_count() >= 2,
                );

                assert_eq!(script.acknowledged_present_count(), 1);
                assert_eq!(script.pending_present_count(), 1);

                script.push_event(PlatformEvent::Quit);
            },
        );

        let frame = outcome
            .script
            .last_presented_frame()
            .expect("desktop run should retain the last presented frame");
        assert_eq!(frame.width, width);
        assert_eq!(frame.height, height);
        assert_eq!(frame.stride, width * 4);
        assert_eq!(frame.pixels.len(), (width * height * 4) as usize);
        assert_eq!(outcome.frame_count, outcome.script.present_count());
        assert!(outcome.frame_count >= 2);
        assert!(!outcome.running);
        assert_eq!(outcome.script.acknowledged_present_count(), 1);
        assert_eq!(outcome.script.pending_present_count(), 1);

        let feedback = outcome
            .script
            .last_present_feedback()
            .expect("scripted feedback should be retained after acknowledgement");
        assert_eq!(feedback.acknowledged_present_count, 1);
        assert_eq!(feedback.sequence, Some(11));
        assert_eq!(feedback.timestamp_ns, Some(22));
        assert_eq!(feedback.crtc_id, Some(33));
    }

    #[test]
    fn queued_present_mode_backpressures_redraws_until_ack_arrives() {
        let outcome = run_desktop_with_controller(
            320,
            180,
            StandalonePresentMode::Queued,
            0,
            move |script| {
                wait_until("initial queued loading present", TEST_TIMEOUT, || {
                    script.present_count() >= 1
                });

                assert_eq!(script.present_count(), 1);
                assert_eq!(script.pending_present_count(), 1);
                assert_eq!(script.acknowledged_present_count(), 0);

                for _ in 0..4 {
                    script.push_event(PlatformEvent::WindowRedraw {
                        handle: scripted_window(),
                    });
                }

                let checkpoint = Instant::now() + Duration::from_millis(75);
                while Instant::now() < checkpoint {
                    assert_eq!(script.present_count(), 1);
                    assert_eq!(script.pending_present_count(), 1);
                    assert_eq!(script.acknowledged_present_count(), 0);
                    thread::sleep(Duration::from_millis(1));
                }

                script.push_present_ack(Some(41), Some(1_000), Some(7));

                wait_until(
                    "desktop frame after queued backpressure clears",
                    TEST_TIMEOUT,
                    || script.present_count() >= 2,
                );

                assert_eq!(script.acknowledged_present_count(), 1);
                script.push_event(PlatformEvent::Quit);
            },
        );

        assert_eq!(outcome.frame_count, outcome.script.present_count());
        assert_eq!(outcome.script.present_count(), 2);
        assert_eq!(outcome.script.acknowledged_present_count(), 1);
        assert_eq!(outcome.script.pending_present_count(), 1);
        assert!(!outcome.running);
    }

    #[test]
    fn timer_fallback_without_feedback_captures_frames_and_exits_cleanly() {
        let width = 320;
        let height = 180;

        let outcome = run_desktop_with_controller(
            width,
            height,
            StandalonePresentMode::Immediate,
            30,
            move |script| {
                wait_until("immediate startup presents", TEST_TIMEOUT, || {
                    script.present_count() >= 2
                });

                assert_eq!(script.acknowledged_present_count(), script.present_count());
                assert!(script.present_ready());

                script.push_event(PlatformEvent::Quit);
            },
        );

        let frame = outcome
            .script
            .last_presented_frame()
            .expect("fallback pacing should still capture the last presented frame");
        assert_eq!(frame.width, width);
        assert_eq!(frame.height, height);
        assert_eq!(frame.stride, width * 4);
        assert_eq!(frame.pixels.len(), (width * height * 4) as usize);
        assert!(outcome.frame_count >= 2);
        assert_eq!(outcome.frame_count, outcome.script.present_count());
        assert_eq!(
            outcome.script.acknowledged_present_count(),
            outcome.script.present_count()
        );
        assert_eq!(outcome.script.pending_present_count(), 0);
        assert!(!outcome.running);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn launcher_fd_backed_feedback_acknowledges_first_queued_present_only_after_event_arrives() {
        let width = 320;
        let height = 180;
        let refresh_hz = 60;
        let pipe = TestPipe::new();

        let outcome = run_launcher_with_runtime_inputs(
            StandaloneConfig::default(),
            queued_feedback_runtime_inputs(width, height, refresh_hz, pipe.read_fd()),
            move |summary, script| {
                assert_launcher_queued_feedback_summary(&summary, width, height, refresh_hz);

                wait_until("first queued launcher present", TEST_TIMEOUT, || {
                    script.present_count() >= 1 && script.last_presented_frame().is_some()
                });

                assert_eq!(script.present_count(), 1);
                assert_eq!(script.pending_present_count(), 1);
                assert_eq!(script.acknowledged_present_count(), 0);
                assert!(!script.present_ready());
                assert!(script.last_present_feedback().is_none());

                let checkpoint = Instant::now() + Duration::from_millis(75);
                while Instant::now() < checkpoint {
                    assert_eq!(script.acknowledged_present_count(), 0);
                    assert_eq!(script.pending_present_count(), 1);
                    thread::sleep(Duration::from_millis(1));
                }

                pipe.write_all(&build_vblank_like_record(
                    TEST_DRM_EVENT_FLIP_COMPLETE,
                    2,
                    5_000,
                    17,
                    9,
                ));

                wait_until("launcher queued acknowledgement", TEST_TIMEOUT, || {
                    script.acknowledged_present_count() >= 1 && script.present_count() >= 2
                });

                assert_eq!(script.acknowledged_present_count(), 1);
                assert_eq!(script.pending_present_count(), 1);

                let feedback = script
                    .last_present_feedback()
                    .expect("fd-backed acknowledgement should be retained after launcher feedback");
                assert_eq!(feedback.acknowledged_present_count, 1);
                assert_eq!(feedback.sequence, Some(17));
                assert_eq!(feedback.timestamp_ns, Some(2_005_000_000));
                assert_eq!(feedback.crtc_id, Some(9));

                script.push_event(PlatformEvent::Quit);
            },
        );

        let frame = outcome
            .script
            .last_presented_frame()
            .expect("launcher queued feedback path should capture the last presented frame");
        assert_eq!(frame.width, width);
        assert_eq!(frame.height, height);
        assert_launcher_queued_feedback_summary(&outcome.summary, width, height, refresh_hz);
        assert!(outcome.script.present_count() >= 2);
        assert_eq!(outcome.script.acknowledged_present_count(), 1);
        assert_eq!(outcome.script.pending_present_count(), 1);
        assert!(!outcome.running);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn launcher_fd_backed_feedback_keeps_redraw_flood_backpressured_until_acknowledged() {
        let width = 320;
        let height = 180;
        let refresh_hz = 60;
        let pipe = TestPipe::new();

        let outcome = run_launcher_with_runtime_inputs(
            StandaloneConfig::default(),
            queued_feedback_runtime_inputs(width, height, refresh_hz, pipe.read_fd()),
            move |summary, script| {
                assert_launcher_queued_feedback_summary(&summary, width, height, refresh_hz);

                wait_until("initial queued launcher present", TEST_TIMEOUT, || {
                    script.present_count() >= 1
                });

                assert_eq!(script.present_count(), 1);
                assert_eq!(script.pending_present_count(), 1);
                assert_eq!(script.acknowledged_present_count(), 0);

                for _ in 0..4 {
                    script.push_event(PlatformEvent::WindowRedraw {
                        handle: scripted_window(),
                    });
                }

                let checkpoint = Instant::now() + Duration::from_millis(75);
                while Instant::now() < checkpoint {
                    assert_eq!(script.present_count(), 1);
                    assert_eq!(script.pending_present_count(), 1);
                    assert_eq!(script.acknowledged_present_count(), 0);
                    thread::sleep(Duration::from_millis(1));
                }

                pipe.write_all(&build_vblank_like_record(
                    TEST_DRM_EVENT_VBLANK,
                    11,
                    125,
                    91,
                    4,
                ));

                wait_until(
                    "launcher frame after queued backpressure clears",
                    TEST_TIMEOUT,
                    || script.present_count() >= 2 && script.acknowledged_present_count() >= 1,
                );

                assert_eq!(script.present_count(), 2);
                assert_eq!(script.acknowledged_present_count(), 1);
                assert_eq!(script.pending_present_count(), 1);

                let settle_deadline = Instant::now() + Duration::from_millis(75);
                while Instant::now() < settle_deadline {
                    assert_eq!(script.present_count(), 2);
                    assert_eq!(script.acknowledged_present_count(), 1);
                    assert_eq!(script.pending_present_count(), 1);
                    thread::sleep(Duration::from_millis(1));
                }

                let feedback = script
                    .last_present_feedback()
                    .expect("fd-backed vblank acknowledgement should be retained");
                assert_eq!(feedback.acknowledged_present_count, 1);
                assert_eq!(feedback.sequence, Some(91));
                assert_eq!(feedback.timestamp_ns, Some(11_000_125_000));
                assert_eq!(feedback.crtc_id, Some(4));

                script.push_event(PlatformEvent::Quit);
            },
        );

        assert_launcher_queued_feedback_summary(&outcome.summary, width, height, refresh_hz);
        assert_eq!(outcome.script.present_count(), 2);
        assert_eq!(outcome.script.acknowledged_present_count(), 1);
        assert_eq!(outcome.script.pending_present_count(), 1);
        assert!(!outcome.running);
    }

    #[test]
    fn launcher_no_output_fallback_uses_default_timer_pacing_and_quits_cleanly() {
        let outcome =
            run_launcher_with_controller(StandaloneConfig::default(), move |_, script| {
                wait_until("launcher fallback frame capture", TEST_TIMEOUT, || {
                    script.last_presented_frame().is_some() && script.present_count() >= 2
                });

                script.push_event(PlatformEvent::Quit);
            });

        let frame = outcome
            .script
            .last_presented_frame()
            .expect("launcher fallback should capture a presented frame before quit");
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(outcome.summary.width, 1920);
        assert_eq!(outcome.summary.height, 1080);
        assert_eq!(outcome.summary.refresh_hz, 60);
        assert_eq!(outcome.summary.requested_fps_cap, 0);
        assert_eq!(outcome.summary.effective_fps_cap, 60);
        assert_eq!(
            outcome.summary.present_mode,
            StandalonePresentMode::Immediate
        );
        assert!(!outcome.summary.live_present_feedback_capable);
        assert!(outcome.summary.output_name.is_none());
        assert_eq!(
            outcome.summary.fallback_reason.geometry,
            Some(StandaloneGeometryFallbackReason::NoOutputMetadata)
        );
        assert_eq!(
            outcome.summary.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );
        assert!(outcome.script.present_count() >= 1);
        assert_eq!(
            outcome.script.acknowledged_present_count(),
            outcome.script.present_count()
        );
        assert_eq!(outcome.script.pending_present_count(), 0);
        assert!(!outcome.running);
    }

    #[test]
    fn launcher_without_drm_keeps_live_present_feedback_disabled() {
        // A freshly-constructed launcher has not yet run `setup_display`, so
        // it cannot observe real page-flip-backed feedback. Both the plan it
        // produces from `current_runtime_inputs()` and the underlying
        // capability flag must stay in the timer-pacing fallback regime.
        let launcher = StandaloneLauncher::new(StandaloneConfig::default());
        let runtime_inputs = StandaloneLaunchRuntimeInputs::from_launcher(&launcher);
        assert!(!runtime_inputs.live_present_feedback_capable);
        assert!(runtime_inputs.present_feedback_fd.is_none());
        assert!(!runtime_inputs.active_live_present_feedback_capability());

        let summary = runtime_inputs.launch_summary(0);
        assert_eq!(summary.present_mode, StandalonePresentMode::Immediate);
        assert!(!summary.live_present_feedback_capable);
        assert_eq!(
            summary.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );
    }

    #[test]
    fn feedback_fd_without_submitter_keeps_queued_present_disabled() {
        let runtime_inputs = StandaloneLaunchRuntimeInputs {
            primary_output: Some(scripted_output("DP-1", 2560, 1440, 144)),
            live_present_feedback_capable: false,
            present_feedback_fd: Some(42),
            ..StandaloneLaunchRuntimeInputs::default()
        };

        assert!(!runtime_inputs.active_live_present_feedback_capability());
        let summary = runtime_inputs.launch_summary(0);
        assert_eq!(summary.present_mode, StandalonePresentMode::Immediate);
        assert_eq!(summary.effective_fps_cap, 144);
        assert_eq!(
            summary.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );
    }

    #[test]
    fn launcher_plan_uses_output_metadata_without_enabling_queued_present_mode() {
        let display = DisplayOutput::from_connectors(&[scripted_connector(
            7,
            "DP-1",
            ConnectorStatus::Connected,
            vec![scripted_mode(
                2560,
                1440,
                144,
                ModeFlags::PREFERRED,
                "2560x1440@144",
            )],
        )]);

        let summary = StandaloneLauncher::build_launch_plan_for_inputs(0, display.primary(), false);

        assert_eq!(summary.width, 2560);
        assert_eq!(summary.height, 1440);
        assert_eq!(summary.refresh_hz, 144);
        assert_eq!(summary.requested_fps_cap, 0);
        assert_eq!(summary.effective_fps_cap, 144);
        assert_eq!(summary.present_mode, StandalonePresentMode::Immediate);
        assert!(!summary.live_present_feedback_capable);
        assert_eq!(summary.output_name.as_deref(), Some("DP-1"));
        assert!(summary.fallback_reason.geometry.is_none());
        assert_eq!(
            summary.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );
    }

    #[test]
    fn launcher_plan_falls_back_when_connector_metadata_is_empty_or_unusable() {
        let display = DisplayOutput::from_connectors(&[
            scripted_connector(
                1,
                "HDMI-A-1",
                ConnectorStatus::Disconnected,
                vec![scripted_mode(
                    1920,
                    1080,
                    60,
                    ModeFlags::PREFERRED,
                    "ignored",
                )],
            ),
            scripted_connector(
                2,
                "DP-2",
                ConnectorStatus::Connected,
                vec![scripted_mode(0, 1440, 144, ModeFlags::CURRENT, "unusable")],
            ),
        ]);

        assert!(display.outputs().is_empty());

        let summary = StandaloneLauncher::build_launch_plan_for_inputs(0, display.primary(), false);

        assert_eq!(summary.width, 1920);
        assert_eq!(summary.height, 1080);
        assert_eq!(summary.refresh_hz, 60);
        assert_eq!(summary.effective_fps_cap, 60);
        assert_eq!(summary.present_mode, StandalonePresentMode::Immediate);
        assert!(!summary.live_present_feedback_capable);
        assert!(summary.output_name.is_none());
        assert_eq!(
            summary.fallback_reason.geometry,
            Some(StandaloneGeometryFallbackReason::NoOutputMetadata)
        );
        assert_eq!(
            summary.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );
    }

    #[test]
    fn launcher_timer_fallback_fps_cap_overrides_or_reuses_selected_refresh_target() {
        let output = scripted_output("DP-2", 3440, 1440, 144);

        let explicit_cap =
            StandaloneLauncher::build_launch_plan_for_inputs(72, Some(&output), false);
        assert_eq!(explicit_cap.refresh_hz, 144);
        assert_eq!(explicit_cap.requested_fps_cap, 72);
        assert_eq!(explicit_cap.effective_fps_cap, 72);
        assert_eq!(explicit_cap.present_mode, StandalonePresentMode::Immediate);
        assert_eq!(
            explicit_cap.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );

        let metadata_refresh =
            StandaloneLauncher::build_launch_plan_for_inputs(0, Some(&output), false);
        assert_eq!(metadata_refresh.refresh_hz, 144);
        assert_eq!(metadata_refresh.requested_fps_cap, 0);
        assert_eq!(metadata_refresh.effective_fps_cap, 144);
        assert_eq!(
            metadata_refresh.present_mode,
            StandalonePresentMode::Immediate
        );

        let default_refresh = StandaloneLauncher::build_launch_plan_for_inputs(0, None, false);
        assert_eq!(default_refresh.refresh_hz, 60);
        assert_eq!(default_refresh.requested_fps_cap, 0);
        assert_eq!(default_refresh.effective_fps_cap, 60);
        assert_eq!(
            default_refresh.present_mode,
            StandalonePresentMode::Immediate
        );
        assert_eq!(
            default_refresh.fallback_reason.geometry,
            Some(StandaloneGeometryFallbackReason::NoOutputMetadata)
        );
        assert_eq!(
            default_refresh.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );
    }
}
