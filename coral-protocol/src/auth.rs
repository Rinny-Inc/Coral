use num_bigint::BigInt;
use serde::Deserialize;
use sha1::{Digest, Sha1};

#[derive(Debug, Clone)]
pub struct AuthProfile {
    pub uuid: String,
    pub username: String,
    pub properties: Vec<ProfileProperty>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

pub fn compute_server_hash(server_id: &str, shared_secret: &[u8], public_key: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key);
    let hash = hasher.finalize();

    let bigint = BigInt::from_signed_bytes_be(&hash);
    format!("{:x}", bigint)
}

pub async fn authenticate(username: &str, server_hash: &str) -> Option<AuthProfile> {
    let url = format!(
        "https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}",
        username, server_hash
    );

    let resp = reqwest::get(&url).await.ok()?;
    if resp.status() == 204 || !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let uuid_str = json["id"].as_str()?.to_string();
    let name = json["name"].as_str()?.to_string();

    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &uuid_str[0..8],
        &uuid_str[8..12],
        &uuid_str[12..16],
        &uuid_str[16..20],
        &uuid_str[20..32]
    );

    let properties = json["properties"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|p| ProfileProperty {
            name: p["name"].as_str().unwrap_or("").to_string(),
            value: p["value"].as_str().unwrap_or("").to_string(),
            signature: p["signature"].as_str().map(|s| s.to_string()),
        })
        .collect();
    Some(AuthProfile {
        uuid,
        username: name,
        properties,
    })
}
