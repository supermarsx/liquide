//! Platform-specific storage device enumeration, mount/unmount, and usage queries.
//!
//! Each platform implements the same set of public functions using OS-specific
//! mechanisms:
//!
//! - **Linux**: `/sys/block/` enumeration, `/proc/mounts`, `statvfs()` syscall
//! - **Windows**: PowerShell `Get-Disk`, `Get-Partition`, `Get-Volume`, `Get-PSDrive`
//! - **macOS**: `diskutil list -plist`, `df -B1`, `diskutil mount`/`unmount`

use crate::analyzer::DiskUsage;
use crate::device::{StorageDevice, StorageType};
use crate::error::StorageError;
use crate::filesystem::FileSystem;
use crate::partition::Partition;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;

// ──────────────────────────────────────────────────────────────────────────────
// Linux — /sys/block + /proc/mounts + statvfs()
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Read a sysfs file and return its trimmed contents.
#[cfg(target_os = "linux")]
fn read_sysfs(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Read a sysfs file and parse as u64.
#[cfg(target_os = "linux")]
fn read_sysfs_u64(path: &Path) -> Option<u64> {
    read_sysfs(path).and_then(|s| s.parse::<u64>().ok())
}

/// Mount entry from /proc/mounts.
#[cfg(target_os = "linux")]
struct MountInfo {
    mount_point: String,
    filesystem: String,
}

/// Parse /proc/mounts into a map from device path to mount info.
#[cfg(target_os = "linux")]
fn parse_proc_mounts() -> HashMap<String, MountInfo> {
    let mut mounts = HashMap::new();
    if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                mounts.insert(
                    parts[0].to_string(),
                    MountInfo {
                        mount_point: parts[1].to_string(),
                        filesystem: parts[2].to_string(),
                    },
                );
            }
        }
    }
    mounts
}

/// Query used and available bytes for a mount point via the statvfs() syscall.
///
/// Returns `(used_bytes, available_bytes)`. On failure returns `(0, 0)`.
#[cfg(target_os = "linux")]
fn statvfs_usage(mount_point: &str) -> (u64, u64) {
    // Linux x86-64 statvfs layout (glibc).
    #[repr(C)]
    struct Statvfs {
        f_bsize: u64,
        f_frsize: u64,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_favail: u64,
        f_fsid: u64,
        f_flag: u64,
        f_namemax: u64,
        __spare: [i32; 6],
    }

    unsafe extern "C" {
        fn statvfs(path: *const u8, buf: *mut Statvfs) -> i32;
    }

    let mut path_bytes = mount_point.as_bytes().to_vec();
    path_bytes.push(0); // NUL-terminate for C.

    let mut stat: Statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { statvfs(path_bytes.as_ptr(), &mut stat) };
    if ret != 0 {
        return (0, 0);
    }

    let total = stat.f_blocks * stat.f_frsize;
    let free = stat.f_bfree * stat.f_frsize;
    let used = total.saturating_sub(free);
    let available = stat.f_bavail * stat.f_frsize;
    (used, available)
}

/// Enumerate partitions for a block device by scanning /sys/block/{dev}/{part}/.
#[cfg(target_os = "linux")]
fn list_partitions_for_device(
    dev_name: &str,
    base: &Path,
    mounts: &HashMap<String, MountInfo>,
) -> Vec<Partition> {
    let mut partitions = Vec::new();

    let entries = match std::fs::read_dir(base) {
        Ok(rd) => rd,
        Err(_) => return partitions,
    };

    for entry in entries.flatten() {
        let part_name = entry.file_name().to_string_lossy().to_string();
        // Partition directories start with the parent device name (e.g. sda1 under sda,
        // nvme0n1p1 under nvme0n1).
        if !part_name.starts_with(dev_name) || part_name == dev_name {
            continue;
        }
        let part_path = base.join(&part_name);
        if !part_path.join("size").exists() {
            continue;
        }

        let size_sectors = read_sysfs_u64(&part_path.join("size")).unwrap_or(0);
        let size_bytes = size_sectors * 512;

        let dev_path = format!("/dev/{part_name}");
        let mount_info = mounts.get(&dev_path);

        let (used, available, filesystem, mount_point) = if let Some(mi) = mount_info {
            let usage = statvfs_usage(&mi.mount_point);
            (
                usage.0,
                usage.1,
                mi.filesystem.clone(),
                Some(mi.mount_point.clone()),
            )
        } else {
            (0, 0, String::new(), None)
        };

        let uuid = read_sysfs(&part_path.join("uuid"));
        let is_system =
            mount_point.as_deref() == Some("/") || mount_point.as_deref() == Some("/boot");

        partitions.push(Partition {
            id: dev_path,
            label: None, // Would need blkid for label.
            filesystem: FileSystem::from_str(&filesystem),
            mount_point,
            size_bytes,
            used_bytes: used,
            available_bytes: available,
            uuid,
            is_system,
            is_encrypted: filesystem == "crypto_LUKS",
        });
    }

    partitions
}

#[cfg(target_os = "linux")]
pub fn list_devices() -> Result<Vec<StorageDevice>, StorageError> {
    let block_dir = Path::new("/sys/block");
    let entries = std::fs::read_dir(block_dir)
        .map_err(|e| StorageError::IoError(format!("/sys/block: {e}")))?;

    let mounts = parse_proc_mounts();
    let mut devices = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip virtual / pseudo devices.
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("dm-")
            || name.starts_with("zram")
        {
            continue;
        }

        let base = block_dir.join(&name);

        // Size in 512-byte sectors.
        let size_sectors = read_sysfs_u64(&base.join("size")).unwrap_or(0);
        let size_bytes = size_sectors * 512;
        if size_bytes == 0 {
            continue;
        }

        // Determine device type.
        let rotational = read_sysfs(&base.join("queue/rotational"));
        let removable_flag = read_sysfs(&base.join("removable"));
        let is_removable = removable_flag.as_deref() == Some("1");

        let device_type = if name.starts_with("nvme") {
            StorageType::NVMe
        } else if is_removable {
            StorageType::USB
        } else if rotational.as_deref() == Some("0") {
            StorageType::SSD
        } else {
            StorageType::HDD
        };

        // Model and serial from sysfs.
        let model = read_sysfs(&base.join("device/model"));
        let serial = read_sysfs(&base.join("device/serial"));

        // Enumerate partitions.
        let partitions = list_partitions_for_device(&name, &base, &mounts);

        devices.push(StorageDevice {
            id: format!("/dev/{name}"),
            name: model.clone().unwrap_or_else(|| name.clone()),
            model,
            serial,
            size_bytes,
            device_type,
            removable: is_removable,
            partitions,
        });
    }

    Ok(devices)
}

#[cfg(target_os = "linux")]
pub fn list_partitions() -> Result<Vec<Partition>, StorageError> {
    let devices = list_devices()?;
    Ok(devices
        .into_iter()
        .flat_map(|d| d.partitions)
        .filter(|p| p.mount_point.is_some())
        .collect())
}

#[cfg(target_os = "linux")]
pub fn mount_partition(partition_id: &str, mount_point: &str) -> Result<(), StorageError> {
    // mount/umount are OS operations that inherently require invoking the kernel
    // mount syscall (which in practice needs CAP_SYS_ADMIN).  We use the libc-level
    // mount(2) syscall directly to avoid shelling out.
    use std::ffi::CString;

    let src = CString::new(partition_id)
        .map_err(|_| StorageError::CommandFailed("invalid partition id".into()))?;
    let target = CString::new(mount_point)
        .map_err(|_| StorageError::InvalidMountPoint(mount_point.into()))?;

    // Try common filesystem types.
    for fstype in &["ext4", "ext3", "btrfs", "xfs", "vfat", "ntfs", "exfat"] {
        let fs = CString::new(*fstype).unwrap();
        unsafe extern "C" {
            fn mount(
                source: *const i8,
                target: *const i8,
                filesystemtype: *const i8,
                mountflags: u64,
                data: *const u8,
            ) -> i32;
        }
        let ret = unsafe {
            mount(
                src.as_ptr(),
                target.as_ptr(),
                fs.as_ptr(),
                0,
                std::ptr::null(),
            )
        };
        if ret == 0 {
            return Ok(());
        }
    }

    Err(StorageError::CommandFailed(
        "mount: could not mount with any known filesystem type".into(),
    ))
}

#[cfg(target_os = "linux")]
pub fn unmount_partition(partition_id: &str) -> Result<(), StorageError> {
    use std::ffi::CString;

    // Find the mount point for this partition from /proc/mounts.
    let mounts = parse_proc_mounts();
    let mount_point = mounts
        .get(partition_id)
        .map(|mi| mi.mount_point.clone())
        .ok_or_else(|| StorageError::NotMounted(partition_id.into()))?;

    let target = CString::new(mount_point.as_str())
        .map_err(|_| StorageError::CommandFailed("invalid mount point".into()))?;

    unsafe extern "C" {
        fn umount(target: *const i8) -> i32;
    }

    let ret = unsafe { umount(target.as_ptr()) };
    if ret != 0 {
        return Err(StorageError::CommandFailed(format!(
            "umount {mount_point}: errno {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn eject_device(device_id: &str) -> Result<(), StorageError> {
    // Eject by writing to /sys/block/<dev>/device/delete after unmounting all partitions.
    let dev_name = device_id.strip_prefix("/dev/").unwrap_or(device_id);

    // First unmount all mounted partitions of this device.
    let mounts = parse_proc_mounts();
    for (dev_path, mi) in &mounts {
        if dev_path.starts_with(device_id) && dev_path.len() > device_id.len() {
            let _ = unmount_partition(dev_path);
            let _ = mi; // suppress unused warning
        }
    }

    // Ask the kernel to remove the device.
    let delete_path = format!("/sys/block/{dev_name}/device/delete");
    std::fs::write(&delete_path, "1")
        .map_err(|e| StorageError::CannotEject(format!("{delete_path}: {e}")))?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn query_partition_usage(partition_id: &str) -> Result<(u64, u64), StorageError> {
    let mounts = parse_proc_mounts();
    let mi = mounts
        .get(partition_id)
        .ok_or_else(|| StorageError::NotMounted(partition_id.into()))?;

    let (used, available) = statvfs_usage(&mi.mount_point);
    Ok((used, available))
}

#[cfg(target_os = "linux")]
pub fn disk_usage(path: &str) -> Result<DiskUsage, StorageError> {
    let (used, available) = statvfs_usage(path);
    let total = used + available;
    if total == 0 {
        return Err(StorageError::IoError(format!(
            "statvfs failed for path: {path}"
        )));
    }
    Ok(DiskUsage::from_total_available(total, available))
}

// ──────────────────────────────────────────────────────────────────────────────
// Windows — Win32 API (no PowerShell)
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod win32_storage {
    use std::ffi::c_void;

    unsafe extern "system" {
        pub fn GetLogicalDriveStringsW(len: u32, buf: *mut u16) -> u32;
        pub fn GetDiskFreeSpaceExW(
            dir: *const u16,
            free_avail: *mut u64,
            total: *mut u64,
            total_free: *mut u64,
        ) -> i32;
        pub fn GetVolumeInformationW(
            root: *const u16,
            vol_name: *mut u16,
            vol_name_size: u32,
            serial: *mut u32,
            max_comp: *mut u32,
            flags: *mut u32,
            fs_name: *mut u16,
            fs_name_size: u32,
        ) -> i32;
        pub fn GetDriveTypeW(root: *const u16) -> u32;
        pub fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const c_void,
            creation: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        pub fn DeviceIoControl(
            device: *mut c_void,
            code: u32,
            in_buf: *const c_void,
            in_size: u32,
            out_buf: *mut c_void,
            out_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        pub fn CloseHandle(handle: *mut c_void) -> i32;
    }

    pub const DRIVE_REMOVABLE: u32 = 2;
    pub const DRIVE_FIXED: u32 = 3;
    pub const DRIVE_REMOTE: u32 = 4;
    pub const DRIVE_CDROM: u32 = 5;

    pub const INVALID_HANDLE_VALUE: *mut c_void = -1_isize as *mut c_void;
    pub const OPEN_EXISTING: u32 = 3;
    pub const FILE_SHARE_READ: u32 = 0x00000001;
    pub const FILE_SHARE_WRITE: u32 = 0x00000002;

    // IOCTL_STORAGE_QUERY_PROPERTY
    pub const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;

    // StorageDeviceTrimProperty = 8
    #[repr(C)]
    pub struct StoragePropertyQuery {
        pub property_id: u32,
        pub query_type: u32, // PropertyStandardQuery = 0
        pub additional: [u8; 1],
    }

    #[repr(C)]
    pub struct DeviceTrimDescriptor {
        pub version: u32,
        pub size: u32,
        pub trim_enabled: u8,
    }

    /// Encode a Rust &str to a null-terminated wide string.
    pub fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// Decode a wide string buffer (fixed-length) to a Rust String,
    /// trimming the null terminator and trailing whitespace.
    pub fn wide_buf_to_string(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end]).trim().to_string()
    }
}

#[cfg(target_os = "windows")]
fn enumerate_drive_roots() -> Vec<String> {
    use win32_storage::*;

    let mut buf = vec![0u16; 512];
    let len = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) };
    if len == 0 {
        return Vec::new();
    }

    // The buffer contains null-terminated strings, double-null at the end.
    // e.g. "C:\\\0D:\\\0\0"
    let mut drives = Vec::new();
    let mut start = 0usize;
    let total = len as usize;
    for i in 0..total {
        if buf[i] == 0 {
            if i > start {
                let s = String::from_utf16_lossy(&buf[start..i]);
                drives.push(s);
            }
            start = i + 1;
        }
    }
    drives
}

/// Query whether the physical drive backing a given drive letter supports TRIM
/// (i.e., is likely an SSD/NVMe).
#[cfg(target_os = "windows")]
fn drive_supports_trim(drive_letter: char) -> bool {
    use std::ffi::c_void;
    use win32_storage::*;

    // We need to open \\.\X: (the volume) to query storage properties.
    let vol_path = to_wide(&format!("\\\\.\\{drive_letter}:"));
    let handle = unsafe {
        CreateFileW(
            vol_path.as_ptr(),
            0, // No read/write access needed for the query
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return false;
    }

    // Query StorageDeviceTrimProperty (property_id = 8)
    let query = StoragePropertyQuery {
        property_id: 8, // StorageDeviceTrimProperty
        query_type: 0,  // PropertyStandardQuery
        additional: [0],
    };

    let mut descriptor = DeviceTrimDescriptor {
        version: 0,
        size: 0,
        trim_enabled: 0,
    };
    let mut returned: u32 = 0;

    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const c_void,
            std::mem::size_of::<StoragePropertyQuery>() as u32,
            &mut descriptor as *mut _ as *mut c_void,
            std::mem::size_of::<DeviceTrimDescriptor>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };

    unsafe { CloseHandle(handle) };

    ok != 0 && descriptor.trim_enabled != 0
}

#[cfg(target_os = "windows")]
pub fn list_devices() -> Result<Vec<StorageDevice>, StorageError> {
    use win32_storage::*;

    let roots = enumerate_drive_roots();

    // Group drives by physical drive index. We produce one StorageDevice per
    // unique drive-type class (fixed, removable, remote, cdrom) since without
    // WMI we cannot reliably map drive letters to physical disk numbers.
    // Instead, each logical volume is its own "device" with a single partition.
    let mut devices = Vec::new();

    for root in &roots {
        let root_wide = to_wide(root);
        let drive_type = unsafe { GetDriveTypeW(root_wide.as_ptr()) };

        // Skip unknown / no-root-directory drives.
        if drive_type < 2 {
            continue;
        }

        // Volume label and filesystem name.
        let mut vol_name_buf = [0u16; 256];
        let mut fs_name_buf = [0u16; 64];
        let mut serial: u32 = 0;
        let mut max_comp: u32 = 0;
        let mut flags: u32 = 0;

        let vol_ok = unsafe {
            GetVolumeInformationW(
                root_wide.as_ptr(),
                vol_name_buf.as_mut_ptr(),
                vol_name_buf.len() as u32,
                &mut serial,
                &mut max_comp,
                &mut flags,
                fs_name_buf.as_mut_ptr(),
                fs_name_buf.len() as u32,
            )
        };

        let vol_label = if vol_ok != 0 {
            let s = wide_buf_to_string(&vol_name_buf);
            if s.is_empty() { None } else { Some(s) }
        } else {
            None
        };

        let fs_name = if vol_ok != 0 {
            wide_buf_to_string(&fs_name_buf)
        } else {
            String::from("Unknown")
        };

        // Disk space.
        let mut free_avail: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free: u64 = 0;
        let space_ok = unsafe {
            GetDiskFreeSpaceExW(
                root_wide.as_ptr(),
                &mut free_avail,
                &mut total_bytes,
                &mut total_free,
            )
        };

        let (size_bytes, available_bytes) = if space_ok != 0 {
            (total_bytes, free_avail)
        } else {
            (0, 0)
        };
        let used_bytes = size_bytes.saturating_sub(available_bytes);

        // Drive letter (e.g. 'C').
        let letter = root.chars().next().unwrap_or('?');
        let drive_id = format!("{letter}:");

        // Classify device type.
        let (device_type, removable) = match drive_type {
            DRIVE_REMOVABLE => (StorageType::USB, true),
            DRIVE_REMOTE => (StorageType::NetworkDrive, false),
            DRIVE_CDROM => (StorageType::Optical, true),
            DRIVE_FIXED => {
                // Check TRIM support to distinguish SSD from HDD.
                if drive_supports_trim(letter) {
                    (StorageType::SSD, false)
                } else {
                    (StorageType::HDD, false)
                }
            }
            _ => (StorageType::HDD, false),
        };

        // Determine if this is the system partition (where Windows is installed).
        let is_system = {
            let sys_root =
                std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
            sys_root
                .to_uppercase()
                .starts_with(&letter.to_uppercase().to_string())
        };

        let display_name = match &vol_label {
            Some(lbl) => format!("{lbl} ({drive_id})"),
            None => format!("Volume ({drive_id})"),
        };

        let partition = Partition {
            id: drive_id.clone(),
            label: vol_label,
            filesystem: FileSystem::from_str(&fs_name),
            mount_point: Some(root.clone()),
            size_bytes,
            used_bytes,
            available_bytes,
            uuid: None,
            is_system,
            is_encrypted: false,
        };

        devices.push(StorageDevice {
            id: drive_id,
            name: display_name,
            model: None,
            serial: if serial != 0 {
                Some(format!("{serial:08X}"))
            } else {
                None
            },
            size_bytes,
            device_type,
            removable,
            partitions: vec![partition],
        });
    }

    Ok(devices)
}

#[cfg(target_os = "windows")]
pub fn list_partitions() -> Result<Vec<Partition>, StorageError> {
    let devices = list_devices()?;
    Ok(devices
        .into_iter()
        .flat_map(|d| d.partitions)
        .filter(|p| p.mount_point.is_some())
        .collect())
}

#[cfg(target_os = "windows")]
pub fn mount_partition(_partition_id: &str, _mount_point: &str) -> Result<(), StorageError> {
    // Windows auto-mounts partitions with drive letters. Manual mount-point
    // assignment requires diskpart or mountvol which need elevation.
    Err(StorageError::NotSupported)
}

#[cfg(target_os = "windows")]
pub fn unmount_partition(partition_id: &str) -> Result<(), StorageError> {
    // Use mountvol to dismount a drive letter.
    let output = Command::new("mountvol")
        .args([partition_id, "/P"])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("mountvol: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("mountvol: {stderr}")));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn eject_device(_device_id: &str) -> Result<(), StorageError> {
    // Safe eject requires IOCTL_STORAGE_EJECT_MEDIA which needs elevation.
    // For now, delegate to mountvol to dismount all partitions on the device.
    Err(StorageError::NotSupported)
}

#[cfg(target_os = "windows")]
pub fn query_partition_usage(partition_id: &str) -> Result<(u64, u64), StorageError> {
    use win32_storage::*;

    // partition_id is like "C:" — build root path "C:\".
    let root = if partition_id.ends_with('\\') {
        partition_id.to_string()
    } else {
        format!("{partition_id}\\")
    };
    let root_wide = to_wide(&root);

    let mut free_avail: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;

    let ok = unsafe {
        GetDiskFreeSpaceExW(
            root_wide.as_ptr(),
            &mut free_avail,
            &mut total,
            &mut total_free,
        )
    };

    if ok == 0 {
        return Err(StorageError::CommandFailed(format!(
            "GetDiskFreeSpaceExW failed for {partition_id}"
        )));
    }

    Ok((total, free_avail))
}

#[cfg(target_os = "windows")]
pub fn disk_usage(path: &str) -> Result<DiskUsage, StorageError> {
    // Extract the drive root from the path.
    let drive_root = if path.len() >= 2 && path.as_bytes()[1] == b':' {
        &path[..2]
    } else {
        return Err(StorageError::ParseError(format!(
            "cannot determine drive from path: {path}"
        )));
    };

    let (total, available) = query_partition_usage(drive_root)?;
    Ok(DiskUsage::from_total_available(total, available))
}

// ──────────────────────────────────────────────────────────────────────────────
// macOS
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
pub fn list_devices() -> Result<Vec<StorageDevice>, StorageError> {
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("diskutil list: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!(
            "diskutil exit {}: {stderr}",
            output.status
        )));
    }

    // Parse plist output to find disk identifiers, then query each with diskutil info.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let disk_ids = parse_diskutil_list_plist(&stdout)?;

    let mut devices = Vec::new();
    for disk_id in &disk_ids {
        match query_diskutil_info(disk_id) {
            Ok(dev) => devices.push(dev),
            Err(_) => continue,
        }
    }

    Ok(devices)
}

#[cfg(target_os = "macos")]
fn parse_diskutil_list_plist(plist_str: &str) -> Result<Vec<String>, StorageError> {
    // Simple extraction of disk identifiers from plist text.
    // Look for <string>diskN</string> patterns that are whole-disk identifiers.
    let mut disk_ids = Vec::new();
    for line in plist_str.lines() {
        let trimmed = line.trim();
        if let Some(inner) = trimmed.strip_prefix("<string>") {
            if let Some(name) = inner.strip_suffix("</string>") {
                // Whole-disk identifiers like "disk0", "disk1" (no 's' partition suffix).
                if name.starts_with("disk")
                    && name[4..].chars().all(|c| c.is_ascii_digit())
                    && !disk_ids.contains(&name.to_string())
                {
                    disk_ids.push(name.to_string());
                }
            }
        }
    }
    Ok(disk_ids)
}

#[cfg(target_os = "macos")]
fn query_diskutil_info(disk_id: &str) -> Result<StorageDevice, StorageError> {
    let output = Command::new("diskutil")
        .args(["info", "-plist", disk_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("diskutil info: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract key fields from plist with simple string parsing.
    let get_string = |key: &str| -> Option<String> {
        let key_tag = format!("<key>{key}</key>");
        let mut lines = stdout.lines();
        while let Some(line) = lines.next() {
            if line.trim() == key_tag {
                if let Some(next_line) = lines.next() {
                    let trimmed = next_line.trim();
                    if let Some(inner) = trimmed.strip_prefix("<string>") {
                        if let Some(val) = inner.strip_suffix("</string>") {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
        None
    };

    let get_integer = |key: &str| -> Option<u64> {
        let key_tag = format!("<key>{key}</key>");
        let mut lines = stdout.lines();
        while let Some(line) = lines.next() {
            if line.trim() == key_tag {
                if let Some(next_line) = lines.next() {
                    let trimmed = next_line.trim();
                    if let Some(inner) = trimmed.strip_prefix("<integer>") {
                        if let Some(val) = inner.strip_suffix("</integer>") {
                            return val.parse().ok();
                        }
                    }
                }
            }
        }
        None
    };

    let get_bool = |key: &str| -> bool {
        let key_tag = format!("<key>{key}</key>");
        let mut lines = stdout.lines();
        while let Some(line) = lines.next() {
            if line.trim() == key_tag {
                if let Some(next_line) = lines.next() {
                    let trimmed = next_line.trim();
                    return trimmed == "<true/>";
                }
            }
        }
        false
    };

    let dev_id = format!("/dev/{disk_id}");
    let name = get_string("MediaName").unwrap_or_else(|| disk_id.to_string());
    let model = get_string("DeviceModel");
    let size_bytes = get_integer("TotalSize").unwrap_or(0);
    let removable = get_bool("RemovableMedia") || get_bool("Ejectable");
    let protocol = get_string("DeviceProtocol").unwrap_or_default();
    let solid_state = get_bool("SolidState");

    let device_type = if protocol.contains("NVM") {
        StorageType::NVMe
    } else if protocol.contains("USB") {
        StorageType::USB
    } else if solid_state {
        StorageType::SSD
    } else {
        StorageType::HDD
    };

    // Enumerate partitions by listing diskNsN identifiers.
    let partitions = enumerate_macos_partitions(disk_id)?;

    Ok(StorageDevice {
        id: dev_id,
        name,
        model,
        serial: None,
        size_bytes,
        device_type,
        removable,
        partitions,
    })
}

#[cfg(target_os = "macos")]
fn enumerate_macos_partitions(disk_id: &str) -> Result<Vec<Partition>, StorageError> {
    let output = Command::new("diskutil")
        .args(["list", "-plist", disk_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("diskutil list: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut part_ids = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(inner) = trimmed.strip_prefix("<string>") {
            if let Some(name) = inner.strip_suffix("</string>") {
                if name.starts_with(disk_id) && name.len() > disk_id.len() && name.contains('s') {
                    part_ids.push(name.to_string());
                }
            }
        }
    }

    let mut partitions = Vec::new();
    for part_id in &part_ids {
        let info_output = Command::new("diskutil")
            .args(["info", "-plist", part_id])
            .output();

        let info_output = match info_output {
            Ok(o) if o.status.success() => o,
            _ => continue,
        };

        let info_str = String::from_utf8_lossy(&info_output.stdout);

        let get_field = |key: &str| -> Option<String> {
            let key_tag = format!("<key>{key}</key>");
            let mut lines = info_str.lines();
            while let Some(line) = lines.next() {
                if line.trim() == key_tag {
                    if let Some(next_line) = lines.next() {
                        let t = next_line.trim();
                        if let Some(inner) = t.strip_prefix("<string>") {
                            if let Some(val) = inner.strip_suffix("</string>") {
                                return Some(val.to_string());
                            }
                        }
                        if let Some(inner) = t.strip_prefix("<integer>") {
                            if let Some(val) = inner.strip_suffix("</integer>") {
                                return Some(val.to_string());
                            }
                        }
                    }
                }
            }
            None
        };

        let dev_path = format!("/dev/{part_id}");
        let label = get_field("VolumeName");
        let fstype = get_field("FilesystemType")
            .or_else(|| get_field("Type"))
            .unwrap_or_default();
        let mount_point = get_field("MountPoint").filter(|s| !s.is_empty());
        let size = get_field("TotalSize")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let available = get_field("FreeSpace")
            .or_else(|| get_field("APFSContainerFree"))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let uuid = get_field("VolumeUUID");
        let is_system = mount_point.as_deref() == Some("/");

        partitions.push(Partition {
            id: dev_path,
            label,
            filesystem: FileSystem::from_str(&fstype),
            mount_point,
            size_bytes: size,
            used_bytes: size.saturating_sub(available),
            available_bytes: available,
            uuid,
            is_system,
            is_encrypted: fstype.to_lowercase().contains("encrypted"),
        });
    }

    Ok(partitions)
}

#[cfg(target_os = "macos")]
pub fn list_partitions() -> Result<Vec<Partition>, StorageError> {
    let devices = list_devices()?;
    Ok(devices
        .into_iter()
        .flat_map(|d| d.partitions)
        .filter(|p| p.mount_point.is_some())
        .collect())
}

#[cfg(target_os = "macos")]
pub fn mount_partition(partition_id: &str, _mount_point: &str) -> Result<(), StorageError> {
    let output = Command::new("diskutil")
        .args(["mount", partition_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("diskutil mount: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!(
            "diskutil mount: {stderr}"
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn unmount_partition(partition_id: &str) -> Result<(), StorageError> {
    let output = Command::new("diskutil")
        .args(["unmount", partition_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("diskutil unmount: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!(
            "diskutil unmount: {stderr}"
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn eject_device(device_id: &str) -> Result<(), StorageError> {
    let output = Command::new("diskutil")
        .args(["eject", device_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("diskutil eject: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CannotEject(stderr.to_string()));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn query_partition_usage(partition_id: &str) -> Result<(u64, u64), StorageError> {
    let output = Command::new("df")
        .args(["-B1", partition_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("df: {e}")))?;

    // macOS df doesn't support -B1; use plain df and parse 512-byte blocks.
    let output = Command::new("df")
        .arg(partition_id)
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("df: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("df: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            // macOS df reports 512-byte blocks by default.
            let total_blocks = parts[1].parse::<u64>().unwrap_or(0);
            let avail_blocks = parts[3].parse::<u64>().unwrap_or(0);
            return Ok((total_blocks * 512, avail_blocks * 512));
        }
    }

    Err(StorageError::ParseError("no df output".into()))
}

#[cfg(target_os = "macos")]
pub fn disk_usage(path: &str) -> Result<DiskUsage, StorageError> {
    let (total, available) = query_partition_usage(path)?;
    Ok(DiskUsage::from_total_available(total, available))
}

// ──────────────────────────────────────────────────────────────────────────────
// Fallback for other platforms
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn list_devices() -> Result<Vec<StorageDevice>, StorageError> {
    Err(StorageError::NotSupported)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn list_partitions() -> Result<Vec<Partition>, StorageError> {
    Err(StorageError::NotSupported)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn mount_partition(_partition_id: &str, _mount_point: &str) -> Result<(), StorageError> {
    Err(StorageError::NotSupported)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn unmount_partition(_partition_id: &str) -> Result<(), StorageError> {
    Err(StorageError::NotSupported)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn eject_device(_device_id: &str) -> Result<(), StorageError> {
    Err(StorageError::NotSupported)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn query_partition_usage(_partition_id: &str) -> Result<(u64, u64), StorageError> {
    Err(StorageError::NotSupported)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn disk_usage(_path: &str) -> Result<DiskUsage, StorageError> {
    Err(StorageError::NotSupported)
}
