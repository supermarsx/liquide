use crate::global::{self, Global, GlobalId};

/// Registry of Wayland globals advertised to connecting clients.
///
/// Pre-populated with the standard set of globals on construction.
#[derive(Debug)]
pub struct GlobalRegistry {
    globals: Vec<Global>,
    next_id: u32,
}

impl GlobalRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            globals: Vec::new(),
            next_id: 1,
        };
        // Register standard Wayland globals
        registry.register_standard("wl_compositor", 5, global::wl_compositor);
        registry.register_standard("wl_shm", 1, global::wl_shm);
        registry.register_standard("wl_seat", 8, global::wl_seat);
        registry.register_standard("wl_output", 4, global::wl_output);
        registry.register_standard("xdg_wm_base", 5, global::xdg_wm_base);
        registry.register_standard("wl_data_device_manager", 3, global::wl_data_device_manager);
        registry.register_standard("zwp_linux_dmabuf_v1", 4, global::zwp_linux_dmabuf_v1);
        registry
    }

    fn register_standard(
        &mut self,
        _interface: &str,
        version: u32,
        ctor: fn(GlobalId, u32) -> Global,
    ) {
        let id = GlobalId(self.next_id);
        self.next_id += 1;
        self.globals.push(ctor(id, version));
    }

    pub fn register(&mut self, interface: impl Into<String>, version: u32) -> GlobalId {
        let id = GlobalId(self.next_id);
        self.next_id += 1;
        self.globals.push(Global::new(id, interface, version));
        id
    }

    pub fn globals(&self) -> &[Global] {
        &self.globals
    }

    pub fn find(&self, interface: &str) -> Option<&Global> {
        self.globals.iter().find(|g| g.interface == interface)
    }
}

impl Default for GlobalRegistry {
    fn default() -> Self {
        Self::new()
    }
}
