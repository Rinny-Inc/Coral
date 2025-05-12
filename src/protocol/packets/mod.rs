pub mod handshake;
pub mod login;

pub enum Packet {
    KeepAlive,
    Handshake(handshake::PacketHandshake),
    // MOTD
    LoginSuccess(login::PacketLoginSuccess),
    Unknown(Vec<u8>)
}