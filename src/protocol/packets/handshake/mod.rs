pub mod keepalive;

pub struct PacketHandshake {
    protocol_version: u8,
    host_name: String,
    port: u8,
    requested_protocol: EnumProtocol
}

enum EnumProtocol {
    Handshaking,
    Status,
    Login,
    Play
}