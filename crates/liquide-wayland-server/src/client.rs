use std::collections::HashMap;

/// Unique identifier for a connected Wayland client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u32);

/// Connection state of a Wayland client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    Connected,
    Authenticated,
    Active,
    Disconnecting,
    Disconnected,
}

/// The Wayland protocol object type that a server-side ID maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Surface,
    Region,
    Buffer,
    Callback,
    Compositor,
    ShmPool,
    Seat,
    Keyboard,
    Pointer,
    Touch,
    Output,
    XdgWmBase,
    XdgSurface,
    XdgToplevel,
    XdgPopup,
    DataDevice,
}

/// A single Wayland client connection, tracking its protocol objects.
#[derive(Debug)]
pub struct ClientConnection {
    id: ClientId,
    state: ClientState,
    objects: HashMap<u32, ObjectType>,
    next_id: u32,
}

impl ClientConnection {
    pub fn new(id: ClientId) -> Self {
        Self {
            id,
            state: ClientState::Connected,
            objects: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn state(&self) -> ClientState {
        self.state
    }

    pub fn set_state(&mut self, state: ClientState) {
        self.state = state;
    }

    pub fn allocate_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn register_object(&mut self, id: u32, obj_type: ObjectType) {
        self.objects.insert(id, obj_type);
    }

    pub fn remove_object(&mut self, id: u32) -> Option<ObjectType> {
        self.objects.remove(&id)
    }

    pub fn object_type(&self, id: u32) -> Option<&ObjectType> {
        self.objects.get(&id)
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
}
