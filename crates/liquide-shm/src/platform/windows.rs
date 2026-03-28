use crate::{ShmAccess, ShmHandle, SharedMemoryError, SharedMemoryOps};

pub struct SharedMemory {
    name: String,
    ptr: *mut u8,
    size: usize,
    handle: isize, // HANDLE
    /// Kept for API symmetry with Unix (owner created, non-owner opened).
    /// On Windows the kernel reference-counts section objects automatically.
    #[allow(dead_code)]
    is_owner: bool,
}

// Safety: SharedMemory is conceptually an owned memory region.
// The pointer is valid for the lifetime of the mapping and access
// is controlled by the caller (single-writer or external sync).
unsafe impl Send for SharedMemory {}

// Win32 FFI
unsafe extern "system" {
    fn CreateFileMappingW(
        hFile: isize,
        lpFileMappingAttributes: *const u8,
        flProtect: u32,
        dwMaximumSizeHigh: u32,
        dwMaximumSizeLow: u32,
        lpName: *const u16,
    ) -> isize;
    fn OpenFileMappingW(
        dwDesiredAccess: u32,
        bInheritHandle: i32,
        lpName: *const u16,
    ) -> isize;
    fn MapViewOfFile(
        hFileMappingObject: isize,
        dwDesiredAccess: u32,
        dwFileOffsetHigh: u32,
        dwFileOffsetLow: u32,
        dwNumberOfBytesToMap: usize,
    ) -> *mut u8;
    fn UnmapViewOfFile(lpBaseAddress: *const u8) -> i32;
    fn CloseHandle(hObject: isize) -> i32;
    fn GetLastError() -> u32;
    fn VirtualQuery(
        lpAddress: *const u8,
        lpBuffer: *mut MemoryBasicInformation,
        dwLength: usize,
    ) -> usize;
}

#[repr(C)]
struct MemoryBasicInformation {
    base_address: *const u8,
    allocation_base: *const u8,
    allocation_protect: u32,
    _partition_id: u16,
    _pad: [u8; 2],
    region_size: usize,
    state: u32,
    protect: u32,
    type_: u32,
}

const INVALID_HANDLE_VALUE: isize = -1;
const PAGE_READWRITE: u32 = 0x04;
const FILE_MAP_ALL_ACCESS: u32 = 0xF001F;
const FILE_MAP_READ: u32 = 0x04;
const ERROR_ALREADY_EXISTS: u32 = 183;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl SharedMemoryOps for SharedMemory {
    fn create(name: &str, size: usize) -> Result<Self, SharedMemoryError> {
        let wide_name = to_wide(name);
        unsafe {
            let handle = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE,
                (size >> 32) as u32,
                size as u32,
                wide_name.as_ptr(),
            );
            if handle == 0 {
                return Err(SharedMemoryError::CreationFailed(format!(
                    "CreateFileMappingW failed: {}",
                    GetLastError()
                )));
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                return Err(SharedMemoryError::AlreadyExists(name.into()));
            }

            let ptr = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size);
            if ptr.is_null() {
                CloseHandle(handle);
                return Err(SharedMemoryError::MapFailed(format!(
                    "MapViewOfFile failed: {}",
                    GetLastError()
                )));
            }

            Ok(SharedMemory {
                name: name.into(),
                ptr,
                size,
                handle,
                is_owner: true,
            })
        }
    }

    fn open(name: &str, access: ShmAccess) -> Result<Self, SharedMemoryError> {
        let wide_name = to_wide(name);
        let desired_access = match access {
            ShmAccess::ReadOnly => FILE_MAP_READ,
            ShmAccess::ReadWrite => FILE_MAP_ALL_ACCESS,
        };
        unsafe {
            let handle = OpenFileMappingW(desired_access, 0, wide_name.as_ptr());
            if handle == 0 {
                return Err(SharedMemoryError::OpenFailed(format!(
                    "OpenFileMappingW failed: {}",
                    GetLastError()
                )));
            }

            let ptr = MapViewOfFile(handle, desired_access, 0, 0, 0);
            if ptr.is_null() {
                CloseHandle(handle);
                return Err(SharedMemoryError::MapFailed(format!(
                    "MapViewOfFile failed: {}",
                    GetLastError()
                )));
            }

            // Query actual size via VirtualQuery
            let mut mbi: MemoryBasicInformation = std::mem::zeroed();
            VirtualQuery(
                ptr,
                &mut mbi,
                std::mem::size_of::<MemoryBasicInformation>(),
            );
            let size = mbi.region_size;

            Ok(SharedMemory {
                name: name.into(),
                ptr,
                size,
                handle,
                is_owner: false,
            })
        }
    }

    fn as_ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    fn size(&self) -> usize {
        self.size
    }

    fn handle(&self) -> ShmHandle {
        ShmHandle {
            name: self.name.clone(),
            size: self.size,
        }
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                UnmapViewOfFile(self.ptr as *const u8);
            }
            if self.handle != 0 {
                CloseHandle(self.handle);
            }
        }
    }
}
