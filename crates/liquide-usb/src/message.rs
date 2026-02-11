//! USB protocol messages for device redirection.

use serde::{Serialize, Deserialize};
use crate::device::DeviceClass;

/// Raw device redirection PDU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRedirectionMsg {
    pub class_id: u8,
    pub device_instance: u32,
    pub pdu_type: u16,
    pub payload: Vec<u8>,
    pub request_id: Option<u32>,
}

/// Request to attach a USB device to the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbAttachRequest {
    pub vid_pid: String,
    pub device_class: DeviceClass,
    pub name: String,
    pub serial: Option<String>,
}

/// Response to a USB attach request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbAttachResponse {
    pub allowed: bool,
    pub reason: Option<String>,
}

/// Reason for detaching a USB device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetachReason {
    UserDisconnected,
    SessionEnded,
    PolicyViolation,
    Error,
}

/// Notification that a USB device has been detached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDetachNotification {
    pub device_instance: u32,
    pub reason: DetachReason,
}

/// USB data transfer between client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDataTransfer {
    pub device_instance: u32,
    pub endpoint: u8,
    pub data: Vec<u8>,
}

/// File system forwarding capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsCapability {
    pub read: bool,
    pub write: bool,
    pub max_file_size: u64,
}

/// Printer forwarding capabilities (stub).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterCapability {}

/// Smart card forwarding capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCardCapability {
    pub max_readers: u32,
    pub pin_pad_support: bool,
}

/// Capability announcement message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityAnnouncement {
    pub fs: Option<FsCapability>,
    pub printer: Option<PrinterCapability>,
    pub smartcard: Option<SmartCardCapability>,
}
