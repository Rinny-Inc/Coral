// Regression: untrusted input in the packet reader must not crash the server.
use coral_protocol::packets::PacketIn;
use coral_protocol::packets::login::EncryptionResponse;
use coral_protocol::reader::Reader;

#[test]
fn read_bytes_past_end_is_clamped_not_panics() {
    let data = [0x01u8, 0x02, 0x03];
    let mut reader = Reader::new(&data);
    // A length far past the end must clamp to what is available, not panic.
    let out = reader.read_bytes(1000);
    assert_eq!(out, vec![0x01, 0x02, 0x03]);
}

#[test]
fn read_string_invalid_utf8_is_lossy_not_panics() {
    // A length-1 string with byte 0xFF is invalid UTF-8.
    let data = [0x01u8, 0xFF];
    let mut reader = Reader::new(&data);
    let s = reader.read_string();
    // Lossy decoding yields the replacement character, no panic.
    assert_eq!(s, "\u{FFFD}");
}

#[test]
fn encryption_response_decode_oversized_length_is_ok() {
    // A malicious client sends an oversized secret length (VarInt 1000) with no
    // following data. Decoding must not panic.
    let mut buf = bytes::Bytes::from(vec![0xE8, 0x07]);
    let decoded = EncryptionResponse::decode(&mut buf).expect("decode should not error");
    // The clamped read yields an empty secret rather than crashing.
    assert!(decoded.shared_secret.is_empty());
}
