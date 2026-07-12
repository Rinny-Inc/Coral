use std::{net::SocketAddr, sync::Arc};

use coral_config::Config;
use coral_protocol::{
    auth::{AuthProfile, ProfileProperty, authenticate, compute_server_hash},
    encryption::{Encryption, decrypt_rsa, generate_verify_token},
    packets::{
        handshake::{EnumProtocol, PacketHandshake},
        login::{EncryptionRequest, EncryptionResponse, LoginStart},
        status::{Ping, Pong, Request, Response},
    },
};
use coral_server::{
    banlist::BanList, bungee::BungeeForwardedData, ops::OpsFile, player::registry::PlayerRegistry,
    whitelist::WhitelistFile,
};
use coral_types::offline_uuid;
use rsa::RsaPrivateKey;
use tokio::{net::TcpStream, sync::RwLock};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use uuid::Uuid;

use crate::{
    Channels,
    codec::{Codec, is_normal_disconnect, kick, send_packet, state::throttle},
};

const ALLOWED_PROTOCOLS: &[i32] = &[/*5,*/ 47];

pub struct PrePlayContext {
    pub player_registry: Arc<PlayerRegistry>,
    pub config: Arc<Config>,
    pub server_icon: Arc<Option<String>>,
    pub banlist: Arc<RwLock<BanList>>,
    pub whitelist: Arc<RwLock<WhitelistFile>>,
    pub ops: Arc<RwLock<OpsFile>>,
    pub private_key: Arc<RsaPrivateKey>,
    pub public_key_der: Arc<Vec<u8>>,
    pub channels: Channels,
    pub bungee_adresses: Arc<Vec<String>>,
    pub connection_throttle: Arc<throttle::ConnectionThrottle>,
}

pub struct JoinRequest {
    pub uuid: Uuid,
    pub profile: AuthProfile,
    pub client_protocol: i32,
    pub peer_ip: Option<SocketAddr>,
    pub is_op: bool,
}

pub async fn pre_play(
    framed: &mut Framed<TcpStream, Codec>,
    ctx: PrePlayContext,
    peer_ip: Option<SocketAddr>,
) -> Option<JoinRequest> {
    let PrePlayContext {
        player_registry,
        config,
        server_icon,
        banlist,
        whitelist,
        private_key,
        public_key_der,
        channels,
        bungee_adresses,
        connection_throttle,
        ops,
    } = ctx;
    let verify_token = generate_verify_token();
    let mut shutdown_rx = channels.shutdown_tx.subscribe();
    let mut pending_username: Option<String> = None;
    let mut client_protocol = -1;

    let mut forwarded_ip: Option<std::net::IpAddr> = None;
    let mut forwarded_uuid: Option<Uuid> = None;
    let mut forwarded_properties: Vec<ProfileProperty> = vec![];

    loop {
        tokio::select! {
                Ok(()) = shutdown_rx.recv() => {
                    if framed.codec().state == EnumProtocol::Login {
                        kick(framed, "Server closed.").await;
                    }
                    return None;
                }
                result = framed.next() => {
                    let result = result?;

                    match result {
                        Ok(packet) => {
                            match framed.codec().state {
                                EnumProtocol::Handshaking => {
                                    if let Some(handshake) = packet.as_any().downcast_ref::<PacketHandshake>() {
                                        client_protocol = handshake.protocol_version;

                                        // bungee ip forwarding
                                        // only trust forwarded if the connection comes from a known proxy IP
                                        let is_proxied = if config.bungee.enabled {
                                            let peer_addr = peer_ip
                                                .map(|a| a.ip().to_string())
                                                .unwrap_or_default();
                                            bungee_adresses.iter().any(|a| a == &peer_addr)
                                        } else {
                                            false
                                        };

                                        if is_proxied {
                                            match BungeeForwardedData::try_from(handshake.host_name.as_str()) {
                                                Ok(data) => {
                                                    forwarded_ip = Some(data.ip);
                                                    forwarded_uuid = Some(data.uuid);
                                                    forwarded_properties = data.properties;
                                                }
                                                Err(e) => {
                                                    eprintln!("Invalid bungee forwarding data, rejecting connection! {:?}", e);
                                                    return None;
                                                }
                                            }
                                        }
                                        framed.codec_mut().state = handshake.requested_protocol.clone();
                                    }
                                }
                                EnumProtocol::Status => {
                                    if packet.as_any().downcast_ref::<Request>().is_some() {
                                        let players = player_registry.get_all().await;
                                        let sample: Vec<(&str, String)> = players.iter()
                                            .take(config.server.player_sample_size as usize)
                                            .map(|p| (p.username.as_str(), p.uuid.hyphenated().to_string()))
                                            .collect();
                                        let sample_refs: Vec<(&str, &str)> = sample.iter()
                                            .map(|(name, uuid)| (*name, uuid.as_str()))
                                            .collect();

                                        let server_protocol = if ALLOWED_PROTOCOLS.contains(&client_protocol) {
                                            client_protocol
                                        } else {
                                            -1
                                        };

                                        send_packet(
                                            framed,
                                            Response::new(
                                                &config.server.motd,
                                                player_registry.get_online_count().await,
                                                config.server.max_players,
                                                server_protocol,
                                                server_icon.as_deref(),
                                                &sample_refs
                                            ),
                                        )
                                        .await;
                                        continue;
                                    }
                                    if let Some(ping) = packet.as_any().downcast_ref::<Ping>() {
                                        send_packet(framed, Pong {
                                            time: ping.time
                                        }).await;
                                        continue;
                                    }
                                }
                                EnumProtocol::Login => {
                                    if !ALLOWED_PROTOCOLS.contains(&client_protocol) {
                                        kick(framed, "Unsupported version. Use 1.8.x").await;
                                        return None;
                                    }
                                    if player_registry.get_online_count().await > config.server.max_players {
                                        kick(framed, "Server is full!").await;
                                        return None;
                                    }
                                    if let Some(ip) = peer_ip
                                        && let Some(ban) = banlist.read().await.is_ip_banned(&ip.ip())
                                    {
                                        kick(framed, &format!("§cYou are IP banned from this server!\n§7Reason: §f{}", ban.reason)).await;
                                        return None;
                                    }
                                    if let Some(login_start) = packet.as_any().downcast_ref::<LoginStart>() {
                                        if let Some(addr) = peer_ip
                                            && !connection_throttle.check(addr.ip()).await
                                        {
                                            kick(framed, "Connection throttled! Please wait before reconnecting.").await;
                                            return None;
                                        }
                                        if config.server.online_mode {
                                            pending_username = Some(login_start.username.clone());
                                            send_packet(framed, EncryptionRequest {
                                                server_id: "".to_string(),
                                                public_key: public_key_der.to_vec(),
                                                verify_token: verify_token.clone(),
                                            }).await;
                                            continue;
                                        }
                                        let uuid = if config.bungee.enabled {
                                            forwarded_uuid.unwrap_or_else(|| offline_uuid(&login_start.username))
                                        } else {
                                            offline_uuid(&login_start.username)
                                        };

                                        let profile = AuthProfile {
                                            uuid: uuid.to_string(),
                                            username: login_start.username.clone(),
                                            properties: if config.bungee.enabled {
                                                forwarded_properties.clone()
                                            } else {
                                                vec![]
                                            }
                                        };

                                        if !is_allowed_to_join(framed, uuid, banlist, whitelist, &config).await {
                                            return None;
                                        }

                                        let effective_ip = forwarded_ip
                                            .map(|ip| SocketAddr::new(ip, peer_ip.map(|a| a.port()).unwrap_or(0)))
                                            .or(peer_ip);

                                        return Some(JoinRequest {
                                            uuid,
                                            profile,
                                            client_protocol,
                                            peer_ip: effective_ip,
                                            is_op: ops.read().await.is_op(uuid),
                                        });
                                    }
                                    if let Some(enc_resp) = packet.as_any().downcast_ref::<EncryptionResponse>() {
                                        let shared_secret = decrypt_rsa(&private_key, &enc_resp.shared_secret);
                                        let decrypted_token = decrypt_rsa(&private_key, &enc_resp.verify_token);

                                        if decrypted_token != verify_token {
                                            kick(framed, "Encryption Error!").await;
                                            return None;
                                        }
                                        let username = match pending_username.take() {
                                            Some(u) => u,
                                            None => return None
                                        };

                                        let server_hash = compute_server_hash("", &shared_secret, &public_key_der);

                                        let profile = match authenticate(&username, &server_hash).await {
                                            Some(p) => p,
                                            None => {
                                                kick(framed, "Failed to verify username!").await;
                                                return None
                                            }
                                        };
                                        let uuid = Uuid::parse_str(&profile.uuid).unwrap_or_else(|_| Uuid::new_v4());

                                        if !is_allowed_to_join(framed, uuid, banlist, whitelist, &config).await {
                                            return None;
                                        }

                                        framed.codec_mut().encryption = Some(Encryption::new(&shared_secret));

                                        return Some(JoinRequest {
                                            uuid,
                                            profile,
                                            client_protocol,
                                            peer_ip,
                                            is_op: ops.read().await.is_op(uuid)
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            if !is_normal_disconnect(&e) {
                                eprintln!("Error processing packet: {:?}", e);
                            }
                            return None;
                        }
                }
            }
        }
    }
}

async fn is_allowed_to_join(
    framed: &mut Framed<TcpStream, Codec>,
    uuid: Uuid,
    banlist: Arc<RwLock<BanList>>,
    whitelist: Arc<RwLock<WhitelistFile>>,
    config: &Config,
) -> bool {
    if let Some(ban) = banlist.read().await.is_player_banned(&uuid) {
        kick(
            framed,
            &format!("§cYou are banned!\n§7Reason: §f{}", ban.reason),
        )
        .await;
        return false;
    }
    if config.server.whitelisted && !whitelist.read().await.is_whitelisted(uuid) {
        kick(framed, "You're not whitelisted on this server!").await;
        return false;
    }
    true
}
