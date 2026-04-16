/// Unique identifier for a Wayland global object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalId(pub u32);

/// A Wayland global: an interface + version advertised to clients in the registry.
#[derive(Debug, Clone)]
pub struct Global {
    /// Server-assigned global ID.
    pub id: GlobalId,
    /// Protocol interface name (e.g. `"wl_compositor"`).
    pub interface: String,
    /// Maximum supported version of the interface.
    pub version: u32,
}

impl Global {
    pub fn new(id: GlobalId, interface: impl Into<String>, version: u32) -> Self {
        Self {
            id,
            interface: interface.into(),
            version,
        }
    }
}

/// Create a `wl_compositor` global.
pub fn wl_compositor(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_compositor", version)
}

/// Create a `wl_shm` global.
pub fn wl_shm(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_shm", version)
}

/// Create a `wl_seat` global.
pub fn wl_seat(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_seat", version)
}

/// Create a `wl_output` global.
pub fn wl_output(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_output", version)
}

/// Create an `xdg_wm_base` global.
pub fn xdg_wm_base(id: GlobalId, version: u32) -> Global {
    Global::new(id, "xdg_wm_base", version)
}

/// Create a `wl_data_device_manager` global.
pub fn wl_data_device_manager(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_data_device_manager", version)
}

/// Create a `zwp_linux_dmabuf_v1` global.
pub fn zwp_linux_dmabuf_v1(id: GlobalId, version: u32) -> Global {
    Global::new(id, "zwp_linux_dmabuf_v1", version)
}
