use crate::device::DrmDevice;
#[cfg(not(any(test, target_os = "linux")))]
use crate::error::DrmError;
use crate::error::Result;

/// Typed identifier for a DRM/KMS object (CRTC, connector, plane, …).
///
/// Mirrors the `obj_id` slot of `DRM_IOCTL_MODE_ATOMIC` in the Linux
/// kernel uapi: an opaque `u32` namespaced per device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

/// Typed identifier for a DRM/KMS object property.
///
/// Mirrors the `prop_id` slot of `DRM_IOCTL_MODE_ATOMIC` in the Linux
/// kernel uapi: an opaque `u32` referring to a property previously
/// resolved against the target object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyId(pub u32);

/// Bitflags for atomic commit operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicFlags(u32);

impl AtomicFlags {
    pub const NONBLOCK: Self = Self(1 << 0);
    pub const ALLOW_MODESET: Self = Self(1 << 1);
    pub const PAGE_FLIP_EVENT: Self = Self(1 << 2);
    pub const TEST_ONLY: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for AtomicFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for AtomicFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// A single property change within an atomic request.
#[derive(Debug, Clone)]
pub struct PropertyChange {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub value: u64,
}

/// An atomic modesetting request that batches property changes.
#[derive(Debug, Clone)]
pub struct AtomicRequest {
    changes: Vec<PropertyChange>,
}

impl AtomicRequest {
    /// Creates an empty atomic request.
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    /// Adds a property change to this request.
    pub fn add_property(&mut self, object: ObjectId, property: PropertyId, value: u64) {
        self.changes.push(PropertyChange {
            object_id: object,
            property_id: property,
            value,
        });
    }

    /// Returns the list of queued property changes.
    pub fn changes(&self) -> &[PropertyChange] {
        &self.changes
    }

    /// Commits the batched property changes to the DRM device.
    #[cfg(any(test, target_os = "linux"))]
    pub fn commit(&self, device: &DrmDevice, flags: AtomicFlags) -> Result<()> {
        let encoded = encode_atomic_request(&self.changes);
        commit_atomic_via_fd(device.fd(), &encoded, flags, 0)
    }

    #[cfg(not(any(test, target_os = "linux")))]
    pub fn commit(&self, _device: &DrmDevice, _flags: AtomicFlags) -> Result<()> {
        Err(DrmError::NoDevice)
    }
}

impl Default for AtomicRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Argument struct for `DRM_IOCTL_MODE_ATOMIC` mirroring the Linux kernel
/// uapi `struct drm_mode_atomic` layout.
#[cfg(any(test, target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // constructed by the pending DRM_IOCTL_MODE_ATOMIC wiring
pub(crate) struct DrmModeAtomic {
    /// Commit flags (e.g. `NONBLOCK`, `ALLOW_MODESET`, `PAGE_FLIP_EVENT`,
    /// `TEST_ONLY`).
    pub flags: u32,
    /// Number of distinct objects referenced by the request.
    pub count_objs: u32,
    /// User pointer to a `[u32; count_objs]` array of object ids.
    pub objs_ptr: u64,
    /// User pointer to a `[u32; count_objs]` array of per-object
    /// property counts.
    pub count_props_ptr: u64,
    /// User pointer to a flat `u32` array of property ids in object-group
    /// order.
    pub props_ptr: u64,
    /// User pointer to a flat `u64` array of property values in
    /// object-group order, parallel to `props_ptr`.
    pub prop_values_ptr: u64,
    /// Reserved, must be zero.
    pub reserved: u64,
    /// Opaque user-data field returned with the completion event.
    pub user_data: u64,
}

/// Encoded buffer layout for an atomic request, ready to be handed to
/// `DRM_IOCTL_MODE_ATOMIC` via a [`DrmModeAtomic`] arg struct.
#[cfg(any(test, target_os = "linux"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedAtomicRequest {
    pub objs: Vec<u32>,
    pub count_props: Vec<u32>,
    pub props: Vec<u32>,
    pub prop_values: Vec<u64>,
}

/// Pure, host-testable encoder that groups a flat list of
/// [`PropertyChange`] entries into the parallel buffers expected by
/// `DRM_IOCTL_MODE_ATOMIC`.
///
/// Grouping rules:
/// - Objects are emitted in first-appearance order.
/// - Within each group the original change order is preserved, including
///   duplicate `PropertyId`s (the caller is responsible for de-duping).
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn encode_atomic_request(changes: &[PropertyChange]) -> EncodedAtomicRequest {
    let mut objs: Vec<u32> = Vec::new();
    let mut count_props: Vec<u32> = Vec::new();
    let mut props: Vec<u32> = Vec::new();
    let mut prop_values: Vec<u64> = Vec::new();

    // First pass: discover distinct objects in first-appearance order.
    for change in changes {
        if !objs.iter().any(|&id| id == change.object_id.0) {
            objs.push(change.object_id.0);
            count_props.push(0);
        }
    }

    // Second pass: emit groups in first-appearance order, preserving
    // submission order within each group.
    for (idx, &obj) in objs.iter().enumerate() {
        for change in changes.iter().filter(|c| c.object_id.0 == obj) {
            props.push(change.property_id.0);
            prop_values.push(change.value);
            count_props[idx] += 1;
        }
    }

    EncodedAtomicRequest {
        objs,
        count_props,
        props,
        prop_values,
    }
}

/// Owned backing storage for the four parallel arrays referenced by a
/// [`DrmModeAtomic`] ioctl arg. The pointer fields of the returned
/// `DrmModeAtomic` borrow into these `Vec`s, so the caller MUST keep the
/// `OwnedAtomicArrays` alive at least until the ioctl returns.
#[cfg(any(test, target_os = "linux"))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // consumed by the pending DRM_IOCTL_MODE_ATOMIC wiring
pub(crate) struct OwnedAtomicArrays {
    pub objs: Vec<u32>,
    pub count_props: Vec<u32>,
    pub props: Vec<u32>,
    pub prop_values: Vec<u64>,
}

/// Materialize a [`DrmModeAtomic`] ioctl arg from an
/// [`EncodedAtomicRequest`], cloning the four parallel arrays into a
/// fresh [`OwnedAtomicArrays`] and pointing the arg's pointer fields at
/// that owned storage.
///
/// # Safety / lifetime
///
/// The returned `DrmModeAtomic` contains raw `u64`-encoded pointers
/// borrowing into the returned `OwnedAtomicArrays`. The caller MUST
/// keep the `OwnedAtomicArrays` alive at least until the
/// `DRM_IOCTL_MODE_ATOMIC` call returns; dropping it earlier would
/// dangle every pointer field. This function itself performs no
/// syscalls and no `unsafe` dereferences — only an integer-arithmetic
/// pointer-to-`u64` cast.
#[cfg(any(test, target_os = "linux"))]
#[allow(dead_code)] // consumed by the pending DRM_IOCTL_MODE_ATOMIC wiring
pub(crate) fn atomic_args_from_encoded(
    encoded: &EncodedAtomicRequest,
    flags: AtomicFlags,
    user_data: u64,
) -> (DrmModeAtomic, OwnedAtomicArrays) {
    let arrays = OwnedAtomicArrays {
        objs: encoded.objs.clone(),
        count_props: encoded.count_props.clone(),
        props: encoded.props.clone(),
        prop_values: encoded.prop_values.clone(),
    };
    let args = DrmModeAtomic {
        flags: flags.bits(),
        count_objs: arrays.objs.len() as u32,
        objs_ptr: arrays.objs.as_ptr() as u64,
        count_props_ptr: arrays.count_props.as_ptr() as u64,
        props_ptr: arrays.props.as_ptr() as u64,
        prop_values_ptr: arrays.prop_values.as_ptr() as u64,
        reserved: 0,
        user_data,
    };
    (args, arrays)
}

/// `DRM_IOWR(0xBC, sizeof(drm_mode_atomic))` — kernel uapi request
/// number for `DRM_IOCTL_MODE_ATOMIC`.
#[cfg(any(test, target_os = "linux"))]
const DRM_IOCTL_MODE_ATOMIC: core::ffi::c_ulong =
    crate::ioctl::drm_iowr(0xBC, std::mem::size_of::<DrmModeAtomic>());

/// Issue `DRM_IOCTL_MODE_ATOMIC` on `fd` for the property changes encoded
/// in `encoded`, with `flags` and `user_data`.
///
/// The pointer fields of the ioctl arg borrow into a fresh
/// [`OwnedAtomicArrays`] held on the stack of this function — it is
/// guaranteed to outlive the syscall because the arg struct is consumed
/// before this function returns.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn commit_atomic_via_fd(
    fd: i32,
    encoded: &EncodedAtomicRequest,
    flags: AtomicFlags,
    user_data: u64,
) -> Result<()> {
    let (mut args, _arrays) = atomic_args_from_encoded(encoded, flags, user_data);
    crate::ioctl::drm_ioctl(fd, DRM_IOCTL_MODE_ATOMIC, "MODE_ATOMIC", &mut args)
    // `_arrays` drops here, after the ioctl has returned, ensuring all
    // pointer fields in `args` were valid for the duration of the call.
}
