//! Filesystem type enumeration and detection.

/// Known filesystem types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystem {
    NTFS,
    Ext4,
    Ext3,
    Btrfs,
    FAT32,
    ExFAT,
    APFS,
    HFS,
    XFS,
    ZFS,
    Swap,
    Unknown(String),
}

impl FileSystem {
    /// Parse a filesystem name string into the corresponding enum variant.
    ///
    /// Matching is case-insensitive and handles common aliases:
    /// - `"ntfs"` -> `NTFS`
    /// - `"ext4"` -> `Ext4`
    /// - `"vfat"`, `"fat32"` -> `FAT32`
    /// - etc.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ntfs" => Self::NTFS,
            "ext4" => Self::Ext4,
            "ext3" => Self::Ext3,
            "btrfs" => Self::Btrfs,
            "fat32" | "vfat" | "msdos" => Self::FAT32,
            "exfat" => Self::ExFAT,
            "apfs" => Self::APFS,
            "hfs" | "hfs+" | "hfsplus" => Self::HFS,
            "xfs" => Self::XFS,
            "zfs" => Self::ZFS,
            "swap" | "linux-swap" => Self::Swap,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Return the canonical name of this filesystem.
    pub fn name(&self) -> &str {
        match self {
            Self::NTFS => "NTFS",
            Self::Ext4 => "ext4",
            Self::Ext3 => "ext3",
            Self::Btrfs => "btrfs",
            Self::FAT32 => "FAT32",
            Self::ExFAT => "exFAT",
            Self::APFS => "APFS",
            Self::HFS => "HFS+",
            Self::XFS => "XFS",
            Self::ZFS => "ZFS",
            Self::Swap => "swap",
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// Returns `true` if this filesystem supports POSIX permissions.
    pub fn supports_posix_permissions(&self) -> bool {
        matches!(
            self,
            Self::Ext4 | Self::Ext3 | Self::Btrfs | Self::APFS | Self::HFS | Self::XFS | Self::ZFS
        )
    }

    /// Returns `true` if this filesystem supports journaling.
    pub fn supports_journaling(&self) -> bool {
        matches!(
            self,
            Self::NTFS
                | Self::Ext4
                | Self::Ext3
                | Self::Btrfs
                | Self::APFS
                | Self::HFS
                | Self::XFS
                | Self::ZFS
        )
    }
}

impl std::fmt::Display for FileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
