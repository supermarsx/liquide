use crate::device::DeviceClass;
use crate::message::{
    CapabilityAnnouncement, DetachReason, DeviceRedirectionMsg, FsCapability, SmartCardCapability,
    UsbAttachRequest, UsbAttachResponse, UsbDataTransfer, UsbDetachNotification,
};

#[test]
fn test_attach_request_serialize() {
    let req = UsbAttachRequest {
        vid_pid: "046D:C534".to_string(),
        device_class: DeviceClass::Filesystem,
        name: "USB Drive".to_string(),
        serial: Some("SN123".to_string()),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("046D:C534"));
    let decoded: UsbAttachRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.vid_pid, "046D:C534");
}

#[test]
fn test_attach_response_serialize() {
    let resp = UsbAttachResponse {
        allowed: true,
        reason: None,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let decoded: UsbAttachResponse = serde_json::from_str(&json).unwrap();
    assert!(decoded.allowed);
    assert!(decoded.reason.is_none());
}

#[test]
fn test_detach_notification() {
    let notif = UsbDetachNotification {
        device_instance: 42,
        reason: DetachReason::SessionEnded,
    };
    let json = serde_json::to_string(&notif).unwrap();
    let decoded: UsbDetachNotification = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.device_instance, 42);
    assert_eq!(decoded.reason, DetachReason::SessionEnded);
}

#[test]
fn test_data_transfer_serialize() {
    let xfer = UsbDataTransfer {
        device_instance: 1,
        endpoint: 0x81,
        data: vec![0xDE, 0xAD],
    };
    let json = serde_json::to_string(&xfer).unwrap();
    let decoded: UsbDataTransfer = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.endpoint, 0x81);
    assert_eq!(decoded.data, vec![0xDE, 0xAD]);
}

#[test]
fn test_capability_announcement() {
    let cap = CapabilityAnnouncement {
        fs: Some(FsCapability {
            read: true,
            write: false,
            max_file_size: 1024 * 1024,
        }),
        printer: None,
        smartcard: Some(SmartCardCapability {
            max_readers: 4,
            pin_pad_support: true,
        }),
    };
    let json = serde_json::to_string(&cap).unwrap();
    let decoded: CapabilityAnnouncement = serde_json::from_str(&json).unwrap();
    assert!(decoded.fs.is_some());
    assert!(decoded.printer.is_none());
    assert!(decoded.smartcard.is_some());
}
