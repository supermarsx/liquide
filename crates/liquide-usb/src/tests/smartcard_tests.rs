use crate::smartcard::{ApduCommand, SmartCardReader, SmartCardReaderState};

fn make_reader() -> SmartCardReader {
    SmartCardReader::new("Virtual Reader 0".to_string())
}

#[test]
fn test_reader_initial_state() {
    let reader = make_reader();
    assert_eq!(reader.state(), SmartCardReaderState::Idle);
    assert_eq!(reader.name(), "Virtual Reader 0");
    assert!(reader.atr().is_none());
}

#[test]
fn test_reader_insert_remove_card() {
    let mut reader = make_reader();
    let atr = vec![0x3B, 0x8C, 0x80, 0x01];
    reader.insert_card(atr.clone()).unwrap();
    assert_eq!(reader.state(), SmartCardReaderState::CardInserted);
    assert_eq!(reader.atr(), Some(atr.as_slice()));

    reader.remove_card().unwrap();
    assert_eq!(reader.state(), SmartCardReaderState::Idle);
    assert!(reader.atr().is_none());
}

#[test]
fn test_reader_double_insert_fails() {
    let mut reader = make_reader();
    reader.insert_card(vec![0x3B]).unwrap();
    let result = reader.insert_card(vec![0x3B]);
    assert!(result.is_err());
}

#[test]
fn test_reader_apdu_exchange() {
    let mut reader = make_reader();
    reader.insert_card(vec![0x3B, 0x00]).unwrap();

    let cmd = ApduCommand {
        cla: 0x00,
        ins: 0xA4,
        p1: 0x04,
        p2: 0x00,
        data: vec![0xA0, 0x00, 0x00, 0x03, 0x08],
        le: Some(256),
    };
    let resp = reader.exchange_apdu(&cmd).unwrap();
    assert_eq!(resp.sw1, 0x90);
    assert_eq!(resp.sw2, 0x00);
}

#[test]
fn test_reader_apdu_without_card_fails() {
    let mut reader = make_reader();
    let cmd = ApduCommand {
        cla: 0x00,
        ins: 0xA4,
        p1: 0x00,
        p2: 0x00,
        data: Vec::new(),
        le: None,
    };
    let result = reader.exchange_apdu(&cmd);
    assert!(result.is_err());
}

#[test]
fn test_reader_reset() {
    let mut reader = make_reader();
    reader.insert_card(vec![0x3B]).unwrap();
    reader.reset();
    assert_eq!(reader.state(), SmartCardReaderState::Idle);
    assert!(reader.atr().is_none());
}
