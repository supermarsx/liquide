//! Disk and storage device management for the LiquiDE desktop environment.
//!
//! Provides cross-platform storage enumeration, partition management,
//! disk usage analysis, and low-space monitoring.
//!
//! # Platform support
//!
//! - **Linux**: `/sys/block/` enumeration, `/proc/mounts`, `statvfs()` syscall
//! - **Windows**: PowerShell `Get-Disk`, `Get-Partition`, `Get-Volume`
//! - **macOS**: `diskutil`, `df`
//!
//! # Example
//!
//! ```no_run
//! use liquide_storage::{StorageManager, SpaceMonitor, analyzer};
//!
//! let mut mgr = StorageManager::new();
//! // List all physical/virtual storage devices.
//! if let Ok(devices) = mgr.list_devices() {
//!     for dev in &devices {
//!         println!("{} ({}) - {}", dev.name, dev.device_type, dev.formatted_size());
//!     }
//! }
//!
//! // Analyze directory usage.
//! let usage = analyzer::analyze_directory("/home", 2);
//! println!("{}: {}", usage.path, analyzer::format_size(usage.size_bytes));
//! ```

pub mod analyzer;
pub mod device;
pub mod error;
pub mod event;
pub mod filesystem;
pub mod manager;
pub mod monitor;
pub mod partition;
pub mod platform;

// Re-export primary types at crate root for convenience.
pub use analyzer::{DirUsage, DiskUsage, FileInfo};
pub use device::{StorageDevice, StorageType};
pub use error::StorageError;
pub use event::StorageEvent;
pub use filesystem::FileSystem;
pub use manager::StorageManager;
pub use monitor::SpaceMonitor;
pub use partition::Partition;

#[cfg(test)]
mod tests {
    use super::*;

    // ── FileSystem tests ──────────────────────────────────────────────

    #[test]
    fn filesystem_from_str_known_types() {
        assert_eq!(FileSystem::from_str("ntfs"), FileSystem::NTFS);
        assert_eq!(FileSystem::from_str("NTFS"), FileSystem::NTFS);
        assert_eq!(FileSystem::from_str("ext4"), FileSystem::Ext4);
        assert_eq!(FileSystem::from_str("EXT4"), FileSystem::Ext4);
        assert_eq!(FileSystem::from_str("ext3"), FileSystem::Ext3);
        assert_eq!(FileSystem::from_str("btrfs"), FileSystem::Btrfs);
        assert_eq!(FileSystem::from_str("fat32"), FileSystem::FAT32);
        assert_eq!(FileSystem::from_str("vfat"), FileSystem::FAT32);
        assert_eq!(FileSystem::from_str("msdos"), FileSystem::FAT32);
        assert_eq!(FileSystem::from_str("exfat"), FileSystem::ExFAT);
        assert_eq!(FileSystem::from_str("apfs"), FileSystem::APFS);
        assert_eq!(FileSystem::from_str("hfs"), FileSystem::HFS);
        assert_eq!(FileSystem::from_str("hfs+"), FileSystem::HFS);
        assert_eq!(FileSystem::from_str("hfsplus"), FileSystem::HFS);
        assert_eq!(FileSystem::from_str("xfs"), FileSystem::XFS);
        assert_eq!(FileSystem::from_str("zfs"), FileSystem::ZFS);
        assert_eq!(FileSystem::from_str("swap"), FileSystem::Swap);
        assert_eq!(FileSystem::from_str("linux-swap"), FileSystem::Swap);
    }

    #[test]
    fn filesystem_from_str_unknown() {
        let fs = FileSystem::from_str("reiserfs");
        assert_eq!(fs, FileSystem::Unknown("reiserfs".to_string()));
    }

    #[test]
    fn filesystem_name_roundtrip() {
        assert_eq!(FileSystem::NTFS.name(), "NTFS");
        assert_eq!(FileSystem::Ext4.name(), "ext4");
        assert_eq!(FileSystem::FAT32.name(), "FAT32");
        assert_eq!(FileSystem::ExFAT.name(), "exFAT");
        assert_eq!(FileSystem::APFS.name(), "APFS");
        assert_eq!(FileSystem::HFS.name(), "HFS+");
        assert_eq!(FileSystem::Swap.name(), "swap");
    }

    #[test]
    fn filesystem_display() {
        assert_eq!(format!("{}", FileSystem::NTFS), "NTFS");
        assert_eq!(format!("{}", FileSystem::Ext4), "ext4");
        assert_eq!(format!("{}", FileSystem::Unknown("foo".into())), "foo");
    }

    #[test]
    fn filesystem_posix_permissions() {
        assert!(FileSystem::Ext4.supports_posix_permissions());
        assert!(FileSystem::Btrfs.supports_posix_permissions());
        assert!(FileSystem::XFS.supports_posix_permissions());
        assert!(!FileSystem::NTFS.supports_posix_permissions());
        assert!(!FileSystem::FAT32.supports_posix_permissions());
        assert!(!FileSystem::ExFAT.supports_posix_permissions());
    }

    #[test]
    fn filesystem_journaling() {
        assert!(FileSystem::NTFS.supports_journaling());
        assert!(FileSystem::Ext4.supports_journaling());
        assert!(FileSystem::Btrfs.supports_journaling());
        assert!(!FileSystem::FAT32.supports_journaling());
        assert!(!FileSystem::Swap.supports_journaling());
    }

    // ── StorageType tests ─────────────────────────────────────────────

    #[test]
    fn storage_type_from_str() {
        assert_eq!(StorageType::from_str("hdd"), StorageType::HDD);
        assert_eq!(StorageType::from_str("ssd"), StorageType::SSD);
        assert_eq!(StorageType::from_str("nvme"), StorageType::NVMe);
        assert_eq!(StorageType::from_str("usb"), StorageType::USB);
        assert_eq!(StorageType::from_str("sd"), StorageType::SDCard);
        assert_eq!(StorageType::from_str("optical"), StorageType::Optical);
        assert_eq!(StorageType::from_str("network"), StorageType::NetworkDrive);
        assert_eq!(StorageType::from_str("virtual"), StorageType::Virtual);
        assert_eq!(StorageType::from_str("loop"), StorageType::Virtual);
    }

    #[test]
    fn storage_type_name_display() {
        assert_eq!(StorageType::HDD.name(), "HDD");
        assert_eq!(StorageType::SSD.name(), "SSD");
        assert_eq!(StorageType::NVMe.name(), "NVMe");
        assert_eq!(format!("{}", StorageType::USB), "USB");
        assert_eq!(format!("{}", StorageType::NetworkDrive), "Network Drive");
    }

    #[test]
    fn storage_type_removable() {
        assert!(StorageType::USB.is_typically_removable());
        assert!(StorageType::SDCard.is_typically_removable());
        assert!(StorageType::Optical.is_typically_removable());
        assert!(!StorageType::HDD.is_typically_removable());
        assert!(!StorageType::SSD.is_typically_removable());
        assert!(!StorageType::NVMe.is_typically_removable());
    }

    // ── Partition tests ───────────────────────────────────────────────

    #[test]
    fn partition_usage_percent() {
        let p = Partition {
            id: "/dev/sda1".into(),
            label: None,
            filesystem: FileSystem::Ext4,
            mount_point: Some("/".into()),
            size_bytes: 1_000_000,
            used_bytes: 750_000,
            available_bytes: 250_000,
            uuid: None,
            is_system: true,
            is_encrypted: false,
        };
        let pct = p.usage_percent();
        assert!((pct - 75.0).abs() < 0.1);
    }

    #[test]
    fn partition_usage_percent_zero_size() {
        let p = Partition {
            id: "X:".into(),
            label: None,
            filesystem: FileSystem::Unknown("none".into()),
            mount_point: None,
            size_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
            uuid: None,
            is_system: false,
            is_encrypted: false,
        };
        assert_eq!(p.usage_percent(), 0.0);
    }

    #[test]
    fn partition_is_mounted() {
        let mounted = Partition {
            id: "/dev/sda1".into(),
            label: None,
            filesystem: FileSystem::Ext4,
            mount_point: Some("/".into()),
            size_bytes: 100,
            used_bytes: 50,
            available_bytes: 50,
            uuid: None,
            is_system: false,
            is_encrypted: false,
        };
        let unmounted = Partition {
            id: "/dev/sda2".into(),
            label: None,
            filesystem: FileSystem::Ext4,
            mount_point: None,
            size_bytes: 100,
            used_bytes: 0,
            available_bytes: 0,
            uuid: None,
            is_system: false,
            is_encrypted: false,
        };
        assert!(mounted.is_mounted());
        assert!(!unmounted.is_mounted());
    }

    // ── StorageDevice tests ───────────────────────────────────────────

    fn make_test_device() -> StorageDevice {
        StorageDevice {
            id: "/dev/sda".into(),
            name: "Test Disk".into(),
            model: Some("TestModel".into()),
            serial: Some("SN123".into()),
            size_bytes: 500_000_000_000,
            device_type: StorageType::SSD,
            removable: false,
            partitions: vec![
                Partition {
                    id: "/dev/sda1".into(),
                    label: Some("System".into()),
                    filesystem: FileSystem::Ext4,
                    mount_point: Some("/".into()),
                    size_bytes: 400_000_000_000,
                    used_bytes: 200_000_000_000,
                    available_bytes: 200_000_000_000,
                    uuid: Some("abc-123".into()),
                    is_system: true,
                    is_encrypted: false,
                },
                Partition {
                    id: "/dev/sda2".into(),
                    label: Some("Data".into()),
                    filesystem: FileSystem::Ext4,
                    mount_point: Some("/data".into()),
                    size_bytes: 100_000_000_000,
                    used_bytes: 50_000_000_000,
                    available_bytes: 50_000_000_000,
                    uuid: Some("def-456".into()),
                    is_system: false,
                    is_encrypted: false,
                },
            ],
        }
    }

    #[test]
    fn device_total_used_bytes() {
        let dev = make_test_device();
        assert_eq!(dev.total_used_bytes(), 250_000_000_000);
    }

    #[test]
    fn device_total_available_bytes() {
        let dev = make_test_device();
        assert_eq!(dev.total_available_bytes(), 250_000_000_000);
    }

    #[test]
    fn device_has_system_partition() {
        let dev = make_test_device();
        assert!(dev.has_system_partition());
    }

    #[test]
    fn device_is_ejectable() {
        let dev = make_test_device();
        // Not removable, so not ejectable.
        assert!(!dev.is_ejectable());

        let usb = StorageDevice {
            id: "/dev/sdb".into(),
            name: "USB Stick".into(),
            model: None,
            serial: None,
            size_bytes: 32_000_000_000,
            device_type: StorageType::USB,
            removable: true,
            partitions: vec![Partition {
                id: "/dev/sdb1".into(),
                label: Some("DATA".into()),
                filesystem: FileSystem::FAT32,
                mount_point: Some("/media/usb".into()),
                size_bytes: 32_000_000_000,
                used_bytes: 16_000_000_000,
                available_bytes: 16_000_000_000,
                uuid: None,
                is_system: false,
                is_encrypted: false,
            }],
        };
        assert!(usb.is_ejectable());
    }

    #[test]
    fn device_formatted_size() {
        let dev = make_test_device();
        let s = dev.formatted_size();
        assert!(s.contains("GiB"));
    }

    // ── DiskUsage tests ───────────────────────────────────────────────

    #[test]
    fn disk_usage_from_total_available() {
        let du = DiskUsage::from_total_available(1_000_000, 250_000);
        assert_eq!(du.total_bytes, 1_000_000);
        assert_eq!(du.used_bytes, 750_000);
        assert_eq!(du.available_bytes, 250_000);
        assert!((du.usage_percent - 75.0).abs() < 0.1);
    }

    #[test]
    fn disk_usage_zero_total() {
        let du = DiskUsage::from_total_available(0, 0);
        assert_eq!(du.usage_percent, 0.0);
    }

    // ── format_size tests ─────────────────────────────────────────────

    #[test]
    fn format_size_bytes() {
        assert_eq!(analyzer::format_size(0), "0 B");
        assert_eq!(analyzer::format_size(1), "1 B");
        assert_eq!(analyzer::format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_kib() {
        assert_eq!(analyzer::format_size(1024), "1.0 KiB");
        assert_eq!(analyzer::format_size(1536), "1.5 KiB");
    }

    #[test]
    fn format_size_mib() {
        assert_eq!(analyzer::format_size(1_048_576), "1.0 MiB");
    }

    #[test]
    fn format_size_gib() {
        assert_eq!(analyzer::format_size(1_073_741_824), "1.0 GiB");
    }

    #[test]
    fn format_size_tib() {
        assert_eq!(analyzer::format_size(1_099_511_627_776), "1.0 TiB");
    }

    #[test]
    fn format_size_pib() {
        assert_eq!(analyzer::format_size(1_125_899_906_842_624), "1.0 PiB");
    }

    // ── StorageError tests ────────────────────────────────────────────

    #[test]
    fn error_display() {
        assert_eq!(
            StorageError::DeviceNotFound("x".into()).to_string(),
            "device not found: x"
        );
        assert_eq!(
            StorageError::PermissionDenied.to_string(),
            "permission denied"
        );
        assert_eq!(
            StorageError::NotSupported.to_string(),
            "operation not supported on this platform"
        );
        assert_eq!(
            StorageError::CannotEject("busy".into()).to_string(),
            "cannot eject device: busy"
        );
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let storage_err: StorageError = io_err.into();
        assert!(storage_err.to_string().contains("gone"));
    }

    #[test]
    fn error_is_std_error() {
        fn assert_error<T: std::error::Error>() {}
        assert_error::<StorageError>();
    }

    // ── StorageEvent tests ────────────────────────────────────────────

    #[test]
    fn event_classification() {
        let dev = make_test_device();
        let added = StorageEvent::DeviceAdded(dev);
        assert!(added.is_device_event());
        assert!(!added.is_mount_event());
        assert!(!added.is_space_low());

        let removed = StorageEvent::DeviceRemoved("x".into());
        assert!(removed.is_device_event());

        let mounted = StorageEvent::PartitionMounted {
            partition_id: "a".into(),
            mount_point: "/mnt".into(),
        };
        assert!(mounted.is_mount_event());
        assert!(!mounted.is_device_event());

        let unmounted = StorageEvent::PartitionUnmounted("a".into());
        assert!(unmounted.is_mount_event());

        let low = StorageEvent::SpaceLow {
            partition_id: "b".into(),
            available_bytes: 100,
            threshold_bytes: 500,
        };
        assert!(low.is_space_low());
        assert!(!low.is_device_event());
        assert!(!low.is_mount_event());
    }

    #[test]
    fn event_display() {
        let ev = StorageEvent::DeviceRemoved("disk0".into());
        assert_eq!(format!("{ev}"), "device removed: disk0");

        let ev2 = StorageEvent::PartitionMounted {
            partition_id: "/dev/sda1".into(),
            mount_point: "/mnt".into(),
        };
        assert!(format!("{ev2}").contains("/dev/sda1"));
        assert!(format!("{ev2}").contains("/mnt"));

        let ev3 = StorageEvent::PartitionUnmounted("/dev/sdb1".into());
        assert!(format!("{ev3}").contains("unmounted"));
    }

    // ── SpaceMonitor tests ────────────────────────────────────────────

    #[test]
    fn monitor_add_remove_watch() {
        let mut mon = SpaceMonitor::new();
        assert_eq!(mon.watch_count(), 0);

        mon.add_watch("C:", 1_000_000);
        assert_eq!(mon.watch_count(), 1);

        mon.add_watch("D:", 2_000_000);
        assert_eq!(mon.watch_count(), 2);

        // Update existing watch.
        mon.add_watch("C:", 5_000_000);
        assert_eq!(mon.watch_count(), 2);

        // Remove.
        assert!(mon.remove_watch("C:"));
        assert_eq!(mon.watch_count(), 1);

        assert!(!mon.remove_watch("Z:"));
        assert_eq!(mon.watch_count(), 1);
    }

    #[test]
    fn monitor_add_watch_default_threshold() {
        let mut mon = SpaceMonitor::new();
        mon.add_watch_default("C:");
        assert_eq!(mon.watch_count(), 1);
    }

    #[test]
    fn monitor_default_trait() {
        let mon = SpaceMonitor::default();
        assert_eq!(mon.watch_count(), 0);
    }

    // ── StorageManager tests ──────────────────────────────────────────

    #[test]
    fn manager_creation() {
        let mgr = StorageManager::new();
        assert!(mgr.cached_devices().is_empty());
    }

    #[test]
    fn manager_default() {
        let mgr = StorageManager::default();
        assert!(mgr.cached_devices().is_empty());
    }

    #[test]
    fn manager_find_device_empty_cache() {
        let mgr = StorageManager::new();
        assert!(mgr.find_device("/dev/sda").is_none());
    }

    #[test]
    fn manager_find_partition_empty_cache() {
        let mgr = StorageManager::new();
        assert!(mgr.find_partition("/dev/sda1").is_none());
    }

    // ── Analyzer directory tests (use temp dir) ───────────────────────

    #[test]
    fn analyze_empty_directory() {
        let tmp = std::env::temp_dir().join("liquide_storage_test_empty");
        let _ = std::fs::create_dir_all(&tmp);
        // Remove any previous test files.
        for entry in std::fs::read_dir(&tmp).into_iter().flatten() {
            if let Ok(e) = entry {
                let _ = std::fs::remove_file(e.path());
                let _ = std::fs::remove_dir_all(e.path());
            }
        }

        let usage = analyzer::analyze_directory(tmp.to_str().unwrap(), 1);
        assert_eq!(usage.size_bytes, 0);
        assert_eq!(usage.file_count, 0);
        assert!(usage.children.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn analyze_directory_with_files() {
        let tmp = std::env::temp_dir().join("liquide_storage_test_files");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Create some test files.
        std::fs::write(tmp.join("a.txt"), "hello").unwrap(); // 5 bytes
        std::fs::write(tmp.join("b.txt"), "world!").unwrap(); // 6 bytes

        let sub = tmp.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("c.txt"), "12345678").unwrap(); // 8 bytes

        let usage = analyzer::analyze_directory(tmp.to_str().unwrap(), 2);
        assert_eq!(usage.size_bytes, 19);
        assert_eq!(usage.file_count, 3);
        assert_eq!(usage.children.len(), 1);
        assert_eq!(usage.children[0].size_bytes, 8);
        assert_eq!(usage.children[0].file_count, 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn analyze_nonexistent_directory() {
        let usage = analyzer::analyze_directory("/nonexistent/path/abc123", 1);
        assert_eq!(usage.size_bytes, 0);
        assert_eq!(usage.file_count, 0);
    }

    #[test]
    fn largest_files_basic() {
        let tmp = std::env::temp_dir().join("liquide_storage_test_largest");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("small.txt"), "hi").unwrap(); // 2 bytes
        std::fs::write(tmp.join("medium.txt"), "hello world").unwrap(); // 11 bytes
        std::fs::write(tmp.join("large.txt"), "a]".repeat(100)).unwrap(); // 200 bytes

        let files = analyzer::largest_files(tmp.to_str().unwrap(), 2);
        assert_eq!(files.len(), 2);
        // Largest first.
        assert!(files[0].size_bytes >= files[1].size_bytes);
        assert!(files[0].path.contains("large.txt"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn largest_files_count_exceeds_available() {
        let tmp = std::env::temp_dir().join("liquide_storage_test_largest2");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("only.txt"), "one").unwrap();

        let files = analyzer::largest_files(tmp.to_str().unwrap(), 10);
        assert_eq!(files.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn analyze_directory_depth_zero() {
        let tmp = std::env::temp_dir().join("liquide_storage_test_depth0");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let sub = tmp.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(tmp.join("root.txt"), "abc").unwrap();
        std::fs::write(sub.join("child.txt"), "def").unwrap();

        let usage = analyzer::analyze_directory(tmp.to_str().unwrap(), 0);
        // size_bytes still includes recursive total.
        assert_eq!(usage.size_bytes, 6);
        assert_eq!(usage.file_count, 2);
        // But children should be empty at depth 0.
        assert!(usage.children.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Partition formatted helpers ───────────────────────────────────

    #[test]
    fn partition_formatted_helpers() {
        let p = Partition {
            id: "C:".into(),
            label: Some("Windows".into()),
            filesystem: FileSystem::NTFS,
            mount_point: Some("C:\\".into()),
            size_bytes: 500_000_000_000,
            used_bytes: 250_000_000_000,
            available_bytes: 250_000_000_000,
            uuid: None,
            is_system: true,
            is_encrypted: false,
        };
        assert!(p.formatted_size().contains("GiB"));
        assert!(p.formatted_used().contains("GiB"));
        assert!(p.formatted_available().contains("GiB"));
    }

    // ── Default threshold constant ────────────────────────────────────

    #[test]
    fn default_threshold_is_one_gib() {
        assert_eq!(monitor::DEFAULT_THRESHOLD_BYTES, 1024 * 1024 * 1024);
    }
}
