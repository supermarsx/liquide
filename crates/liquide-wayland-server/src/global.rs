#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalId(pub u32);

#[derive(Debug, Clone)]
pub struct Global {
    pub id: GlobalId,
    pub interface: String,
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

pub fn wl_compositor(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_compositor", version)
}

pub fn wl_shm(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_shm", version)
}

pub fn wl_seat(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_seat", version)
}

pub fn wl_output(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_output", version)
}

pub fn xdg_wm_base(id: GlobalId, version: u32) -> Global {
    Global::new(id, "xdg_wm_base", version)
}

pub fn wl_data_device_manager(id: GlobalId, version: u32) -> Global {
    Global::new(id, "wl_data_device_manager", version)
}

pub fn zwp_linux_dmabuf_v1(id: GlobalId, version: u32) -> Global {
    Global::new(id, "zwp_linux_dmabuf_v1", version)
}
