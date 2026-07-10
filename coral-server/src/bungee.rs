use std::net::IpAddr;

use coral_protocol::auth::ProfileProperty;
use uuid::Uuid;

pub struct BungeeForwardedData {
    pub ip: IpAddr,
    pub uuid: Uuid,
    pub properties: Vec<ProfileProperty>,
}

#[derive(Debug)]
pub enum BungeeForwardError {
    PartCount(usize),
    Ip(std::net::AddrParseError),
    Uuid(uuid::Error),
    Properties(serde_json::Error),
}

impl TryFrom<&str> for BungeeForwardedData {
    type Error = BungeeForwardError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut parts: Vec<&str> = value.split('\0').collect();

        if parts.len() == 6 && parts[1] == "FML" {
            parts = vec![parts[0], parts[3], parts[4], parts[5]];
        }

        if parts.len() != 3 && parts.len() != 4 {
            return Err(BungeeForwardError::PartCount(parts.len()));
        }

        // parts[0] = hostname; is discarded
        let ip = parts[1].parse::<IpAddr>().map_err(BungeeForwardError::Ip)?;
        let uuid = Uuid::parse_str(parts[2]).map_err(BungeeForwardError::Uuid)?;
        let properties = match parts.get(3) {
            Some(json) => serde_json::from_str(json).map_err(BungeeForwardError::Properties)?,
            None => Vec::new(),
        };

        Ok(Self {
            ip,
            uuid,
            properties,
        })
    }
}
