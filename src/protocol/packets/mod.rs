use handshake::keepalive;

pub mod handshake;
pub mod login;
pub mod status;

pub enum Packet {
    KeepAlive(keepalive::KeepAlive),
    Handshake(handshake::PacketHandshake),
    StatusStart(status::Start),
    StatusPing(status::Ping),
    StatusDone(status::Done),
    StatusPong(status::Pong),
    LoginSuccess(login::PacketLoginSuccess),
    Unknown(Vec<u8>)
}