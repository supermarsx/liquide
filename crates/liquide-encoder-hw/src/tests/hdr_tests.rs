use crate::hdr::*;

#[test]
fn hdr_format_display() {
    assert_eq!(HdrFormat::Hdr10.to_string(), "HDR10");
    assert_eq!(HdrFormat::Hdr10Plus.to_string(), "HDR10+");
    assert_eq!(HdrFormat::Hlg.to_string(), "HLG");
}

#[test]
fn color_primaries_display() {
    assert_eq!(ColorPrimaries::Bt2020.to_string(), "BT.2020");
}

#[test]
fn pack_sei_nalu_produces_bytes() {
    let meta = HdrMetadata {
        format: HdrFormat::Hdr10,
        primaries: ColorPrimaries::Bt2020,
        transfer: TransferFunction::Pq,
        max_luminance: 1000.0,
        min_luminance: 0.001,
        max_cll: 1000,
        max_fall: 400,
        mastering_display: None,
        dynamic_metadata: None,
    };
    let nalu = meta.pack_sei_nalu();
    assert!(!nalu.is_empty());
    // Should start with start code prefix
    assert_eq!(&nalu[..4], &[0x00, 0x00, 0x00, 0x01]);
}

#[test]
fn pack_obu_metadata_produces_bytes() {
    let meta = HdrMetadata {
        format: HdrFormat::Hlg,
        primaries: ColorPrimaries::Bt709,
        transfer: TransferFunction::Hlg,
        max_luminance: 1000.0,
        min_luminance: 0.0,
        max_cll: 1000,
        max_fall: 400,
        mastering_display: None,
        dynamic_metadata: None,
    };
    let obu = meta.pack_obu_metadata();
    assert!(!obu.is_empty());
}

#[test]
fn tone_map_operator_display() {
    assert_eq!(ToneMapOperator::Aces.to_string(), "ACES");
    assert_eq!(ToneMapOperator::Reinhard.to_string(), "Reinhard");
}
