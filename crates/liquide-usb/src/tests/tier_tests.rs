use crate::config::TierMode;
use crate::tier::{TierCapabilities, TierNegotiator, UsbTier};

fn full_caps() -> TierCapabilities {
    TierCapabilities {
        file_transfer: true,
        smartcard: true,
        full_usb_ip: true,
    }
}

fn limited_caps() -> TierCapabilities {
    TierCapabilities {
        file_transfer: true,
        smartcard: true,
        full_usb_ip: false,
    }
}

fn minimal_caps() -> TierCapabilities {
    TierCapabilities {
        file_transfer: true,
        smartcard: false,
        full_usb_ip: false,
    }
}

#[test]
fn test_tier_auto_full() {
    let tier = TierNegotiator::negotiate(TierMode::Auto, &full_caps(), &full_caps());
    assert_eq!(tier, UsbTier::FullUsbIp);
}

#[test]
fn test_tier_auto_limited() {
    let tier = TierNegotiator::negotiate(TierMode::Auto, &limited_caps(), &full_caps());
    assert_eq!(tier, UsbTier::SmartCard);
}

#[test]
fn test_tier_auto_minimal() {
    let tier = TierNegotiator::negotiate(TierMode::Auto, &minimal_caps(), &full_caps());
    assert_eq!(tier, UsbTier::FileTransfer);
}

#[test]
fn test_tier_forced_tier1() {
    let tier = TierNegotiator::negotiate(TierMode::Tier1, &full_caps(), &full_caps());
    assert_eq!(tier, UsbTier::FileTransfer);
}

#[test]
fn test_tier_forced_tier3() {
    let tier = TierNegotiator::negotiate(TierMode::Tier3, &minimal_caps(), &minimal_caps());
    assert_eq!(tier, UsbTier::FullUsbIp);
}
