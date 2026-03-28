//! Platform-specific storage device enumeration, mount/unmount, and usage queries.
//!
//! Each platform implements the same set of public functions using OS-specific
//! tools:
//!
//! - **Linux**: `lsblk --json`, `df -B1`, `mount`/`umount`, `udisksctl`
//! - **Windows**: PowerShell `Get-Disk`, `Get-Partition`, `Get-Volume`, `Get-PSDrive`
//! - **macOS**: `diskutil list -plist`, `df -B1`, `diskutil mount`/`unmount`

use crate::analyzer::DiskUsage;
use crate::device::{StorageDevice, StorageType};
use crate::error::StorageError;
use crate::filesystem::FileSystem;
use crate::partition::Partition;
use std::process::Command;

// ──────────────────────────────────────────────────────────────────────────────
// Linux
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub fn list_devices() -> Result<Vec<StorageDevice>, StorageError> {
    let output = Command::new("lsblk")
        .args(["--json", "--bytes", "--output",
               "NAME,SIZE,TYPE,MODEL,SERIAL,RM,MOUNTPOINT,FSTYPE,LABEL,UUID,TRAN"])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("lsblk: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("lsblk exit {}: {stderr}", output.status)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_lsblk_json(&stdout)
}

#[cfg(target_os = "linux")]
fn parse_lsblk_json(json_str: &str) -> Result<Vec<StorageDevice>, StorageError> {
    let root: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| StorageError::ParseError(format!("lsblk json: {e}")))?;

    let blockdevices = root["blockdevices"]
        .as_array()
        .ok_or_else(|| StorageError::ParseError("missing blockdevices array".into()))?;

    let mut devices = Vec::new();

    for bd in blockdevices {
        let bd_type = bd["type"].as_str().unwrap_or("");
        if bd_type != "disk" {
            continue;
        }

        let name = bd["name"].as_str().unwrap_or("").to_string();
        let dev_id = format!("/dev/{name}");
        let size_bytes = bd["size"].as_u64().unwrap_or(0);
        let model = bd["model"].as_str().map(|s| s.trim().to_string());
        let serial = bd["serial"].as_str().map(|s| s.trim().to_string());
        let removable = bd["rm"].as_bool().unwrap_or(false)
            || bd["rm"].as_str().map(|s| s == "1").unwrap_or(false)
            || bd["rm"].as_u64().map(|v| v == 1).unwrap_or(false);
        let transport = bd["tran"].as_str().unwrap_or("");

        let device_type = if transport == "nvme" {
            StorageType::NVMe
        } else if transport == "usb" {
            StorageType::USB
        } else if removable && transport.is_empty() {
            StorageType::SDCard
        } else {
            // Heuristic: check rotational flag via model name or default to HDD.
            if model.as_deref().unwrap_or("").to_lowercase().contains("ssd") {
                StorageType::SSD
            } else {
                StorageType::HDD
            }
        };

        // Parse partitions from children.
        let mut partitions = Vec::new();
        if let Some(children) = bd["children"].as_array() {
            for child in children {
                let child_type = child["type"].as_str().unwrap_or("");
                if child_type != "part" {
                    continue;
                }
                let part_name = child["name"].as_str().unwrap_or("");
                let part_id = format!("/dev/{part_name}");
                let part_size = child["size"].as_u64().unwrap_or(0);
                let fstype = child["fstype"].as_str().unwrap_or("");
                let label = child["label"].as_str().map(|s| s.to_string());
                let uuid = child["uuid"].as_str().map(|s| s.to_string());
                let mount_point = child["mountpoint"].as_str().map(|s| s.to_string());

                let is_system = mount_point.as_deref() == Some("/");

                // Query df for used/available if mounted.
                let (used, available) = if mount_point.is_some() {
                    query_partition_usage(&part_id).unwrap_or((part_size, 0))
                } else {
                    (0, 0)
                };
                let used_bytes = part_size.saturating_sub(available.min(part_size));

                partitions.push(Partition {
                    id: part_id,
                    label,
                    filesystem: FileSystem::from_str(fstype),
                    mount_point,
                    size_bytes: part_size,
                    used_bytes: if used > 0 { part_size.saturating_sub(available) } else { used_bytes },
                    available_bytes: available,
                    uuid,
                    is_system,
                    is_encrypted: fstype == "crypto_LUKS",
                });
            }
        }

        devices.push(StorageDevice {
            id: dev_id,
            name: model.clone().unwrap_or_else(|| name.clone()),
            model,
            serial,
            size_bytes,
            device_type,
            removable,
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
    // Try udisksctl first (no root required), fall back to mount.
    let output = Command::new("udisksctl")
        .args(["mount", "--block-device", partition_id, "--mount-options", &format!("mountpoint={mount_point}")])
        .output();

    match output {
        Ok(o) if o.status.success() => return Ok(()),
        _ => {}
    }

    // Fallback: plain mount (requires privileges).
    let output = Command::new("mount")
        .args([partition_id, mount_point])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("mount: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("mount: {stderr}")));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn unmount_partition(partition_id: &str) -> Result<(), StorageError> {
    // Try udisksctl first, fall back to umount.
    let output = Command::new("udisksctl")
        .args(["unmount", "--block-device", partition_id])
        .output();

    match output {
        Ok(o) if o.status.success() => return Ok(()),
        _ => {}
    }

    let output = Command::new("umount")
        .arg(partition_id)
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("umount: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("umount: {stderr}")));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn eject_device(device_id: &str) -> Result<(), StorageError> {
    // Use udisksctl power-off which safely spins down and ejects.
    let output = Command::new("udisksctl")
        .args(["power-off", "--block-device", device_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("udisksctl power-off: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("eject: {stderr}")));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn query_partition_usage(partition_id: &str) -> Result<(u64, u64), StorageError> {
    let output = Command::new("df")
        .args(["-B1", "--output=size,avail", partition_id])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("df: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("df: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip header line.
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let total = parts[0].parse::<u64>().unwrap_or(0);
            let available = parts[1].parse::<u64>().unwrap_or(0);
            return Ok((total, available));
        }
    }

    Err(StorageError::ParseError("no df output".into()))
}

#[cfg(target_os = "linux")]
pub fn disk_usage(path: &str) -> Result<DiskUsage, StorageError> {
    let output = Command::new("df")
        .args(["-B1", "--output=size,used,avail", path])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("df: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("df: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let total = parts[0].parse::<u64>().unwrap_or(0);
            let _used = parts[1].parse::<u64>().unwrap_or(0);
            let available = parts[2].parse::<u64>().unwrap_or(0);
            return Ok(DiskUsage::from_total_available(total, available));
        }
    }

    Err(StorageError::ParseError("no df output".into()))
}

// ──────────────────────────────────────────────────────────────────────────────
// Windows
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub fn list_devices() -> Result<Vec<StorageDevice>, StorageError> {
    let ps_script = r#"
$disks = Get-Disk | Select-Object Number, FriendlyName, SerialNumber, Size, MediaType, BusType, IsOffline, IsReadOnly
$result = @()
foreach ($d in $disks) {
    $parts = Get-Partition -DiskNumber $d.Number -ErrorAction SilentlyContinue |
        Select-Object PartitionNumber, DriveLetter, Size, Type, IsSystem
    $volumes = @()
    foreach ($p in $parts) {
        if ($p.DriveLetter) {
            $vol = Get-Volume -DriveLetter $p.DriveLetter -ErrorAction SilentlyContinue |
                Select-Object FileSystemType, FileSystemLabel, Size, SizeRemaining, HealthStatus
            $volumes += @{
                id = "$($p.DriveLetter):"
                letter = "$($p.DriveLetter)"
                label = if($vol){$vol.FileSystemLabel}else{""}
                fs = if($vol){"$($vol.FileSystemType)"}else{"Unknown"}
                size = $p.Size
                available = if($vol){$vol.SizeRemaining}else{0}
                used = if($vol){$vol.Size - $vol.SizeRemaining}else{0}
                is_system = [bool]$p.IsSystem
            }
        }
    }
    $result += @{
        id = "\\.\PhysicalDrive$($d.Number)"
        name = $d.FriendlyName
        model = $d.FriendlyName
        serial = $d.SerialNumber
        size = $d.Size
        media_type = "$($d.MediaType)"
        bus_type = "$($d.BusType)"
        removable = ($d.BusType -eq 'USB' -or $d.BusType -eq 'SD')
        partitions = $volumes
    }
}
$result | ConvertTo-Json -Depth 4
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("powershell: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("powershell exit {}: {stderr}", output.status)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_windows_devices_json(&stdout)
}

#[cfg(target_os = "windows")]
fn parse_windows_devices_json(json_str: &str) -> Result<Vec<StorageDevice>, StorageError> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // PowerShell may return a single object (not array) if only one disk.
    let values: Vec<serde_json::Value> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)
            .map_err(|e| StorageError::ParseError(format!("json array: {e}")))?
    } else {
        let single: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| StorageError::ParseError(format!("json object: {e}")))?;
        vec![single]
    };

    let mut devices = Vec::new();

    for v in &values {
        let id = v["id"].as_str().unwrap_or("").to_string();
        let name = v["name"].as_str().unwrap_or("Unknown").to_string();
        let model = v["model"].as_str().map(|s| s.to_string());
        let serial = v["serial"].as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string());
        let size_bytes = v["size"].as_u64().unwrap_or(0);
        let media_type = v["media_type"].as_str().unwrap_or("");
        let bus_type = v["bus_type"].as_str().unwrap_or("");
        let removable = v["removable"].as_bool().unwrap_or(false);

        let device_type = match media_type.to_lowercase().as_str() {
            s if s.contains("ssd") => StorageType::SSD,
            s if s.contains("nvme") => StorageType::NVMe,
            _ => match bus_type.to_lowercase().as_str() {
                "nvme" => StorageType::NVMe,
                "usb" => StorageType::USB,
                "sd" => StorageType::SDCard,
                _ => {
                    if media_type.to_lowercase().contains("hdd") || media_type.to_lowercase().contains("unspecified") {
                        StorageType::HDD
                    } else {
                        StorageType::SSD
                    }
                }
            },
        };

        // Parse partitions.
        let mut partitions = Vec::new();
        let parts_val = &v["partitions"];
        let parts_arr: Vec<&serde_json::Value> = if let Some(arr) = parts_val.as_array() {
            arr.iter().collect()
        } else if parts_val.is_object() {
            vec![parts_val]
        } else {
            vec![]
        };

        for pv in &parts_arr {
            let part_id = pv["id"].as_str().unwrap_or("").to_string();
            let label_str = pv["label"].as_str().unwrap_or("");
            let label = if label_str.is_empty() {
                None
            } else {
                Some(label_str.to_string())
            };
            let fs_str = pv["fs"].as_str().unwrap_or("Unknown");
            let part_size = pv["size"].as_u64().unwrap_or(0);
            let available = pv["available"].as_u64().unwrap_or(0);
            let used = pv["used"].as_u64().unwrap_or(part_size.saturating_sub(available));
            let is_system = pv["is_system"].as_bool().unwrap_or(false);

            let mount_point = pv["letter"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| format!("{s}:\\"));

            partitions.push(Partition {
                id: part_id,
                label,
                filesystem: FileSystem::from_str(fs_str),
                mount_point,
                size_bytes: part_size,
                used_bytes: used,
                available_bytes: available,
                uuid: None,
                is_system,
                is_encrypted: false, // Would need BitLocker query.
            });
        }

        devices.push(StorageDevice {
            id,
            name,
            model,
            serial,
            size_bytes,
            device_type,
            removable,
            partitions,
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
pub fn eject_device(device_id: &str) -> Result<(), StorageError> {
    let ps_script = format!(
        r#"
$diskNum = '{device_id}' -replace '.*PhysicalDrive', ''
$disk = Get-Disk -Number $diskNum -ErrorAction Stop
if (-not ($disk.BusType -eq 'USB' -or $disk.BusType -eq 'SD')) {{
    throw "Device is not removable"
}}
# Offline the disk to safely eject
Set-Disk -Number $diskNum -IsOffline $true -ErrorAction Stop
"#
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("powershell eject: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CannotEject(stderr.to_string()));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn query_partition_usage(partition_id: &str) -> Result<(u64, u64), StorageError> {
    // partition_id is like "C:" — extract drive letter.
    let letter = partition_id
        .chars()
        .next()
        .ok_or_else(|| StorageError::PartitionNotFound(partition_id.to_string()))?;

    let ps_script = format!(
        "Get-Volume -DriveLetter '{letter}' | Select-Object Size, SizeRemaining | ConvertTo-Json"
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("powershell: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StorageError::CommandFailed(format!("Get-Volume: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| StorageError::ParseError(format!("json: {e}")))?;

    let total = v["Size"].as_u64().unwrap_or(0);
    let available = v["SizeRemaining"].as_u64().unwrap_or(0);
    Ok((total, available))
}

#[cfg(target_os = "windows")]
pub fn disk_usage(path: &str) -> Result<DiskUsage, StorageError> {
    // Extract the drive root from the path.
    let drive_root = if path.len() >= 2 && path.as_bytes()[1] == b':' {
        &path[..2]
    } else {
        return Err(StorageError::ParseError(format!("cannot determine drive from path: {path}")));
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
        return Err(StorageError::CommandFailed(format!("diskutil exit {}: {stderr}", output.status)));
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
        let fstype = get_field("FilesystemType").or_else(|| get_field("Type")).unwrap_or_default();
        let mount_point = get_field("MountPoint").filter(|s| !s.is_empty());
        let size = get_field("TotalSize").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
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
        return Err(StorageError::CommandFailed(format!("diskutil mount: {stderr}")));
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
        return Err(StorageError::CommandFailed(format!("diskutil unmount: {stderr}")));
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
