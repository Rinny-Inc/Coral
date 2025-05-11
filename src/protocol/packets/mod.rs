pub mod handshake;

pub enum Packet {
    Handshake(handshake::PacketHandshake),
    Unknown(Vec<u8>)
}