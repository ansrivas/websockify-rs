use base64::{engine::general_purpose::STANDARD, Engine as _};
use tokio_wireguard::config::Interface as LocalInterface;
use tokio_wireguard::{config::Peer, interface::ToInterface, Config, Interface};

use std::str::FromStr;
use x25519_dalek::{PublicKey, StaticSecret};

pub struct KeyBytes(pub [u8; 32]);
impl std::str::FromStr for KeyBytes {
    type Err = &'static str;

    /// Can parse a secret key from a hex or base64 encoded string.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut internal = [0u8; 32];

        match s.len() {
            64 => {
                // Try to parse as hex
                for i in 0..32 {
                    internal[i] = u8::from_str_radix(&s[i * 2..=i * 2 + 1], 16)
                        .map_err(|_| "Illegal character in key")?;
                }
            }
            43 | 44 => {
                // Try to parse as base64
                if let Ok(decoded_key) = STANDARD.decode(s) {
                    if decoded_key.len() == internal.len() {
                        internal[..].copy_from_slice(&decoded_key);
                    } else {
                        return Err("Illegal character in key");
                    }
                }
            }
            _ => return Err("Illegal key size"),
        }

        Ok(KeyBytes(internal))
    }
}

pub async fn setup_wireguard_interface() -> Result<Interface, Box<dyn std::error::Error>> {
    let server_private_key = KeyBytes::from_str("xxxx=")?;
    let server_private_key_static_secret = StaticSecret::from(server_private_key.0);

    let peer_public_key = KeyBytes::from_str("xxxx=")?;
    let peer_public_key = PublicKey::from(peer_public_key.0);

    let config = Config {
        interface: LocalInterface {
            private_key: server_private_key_static_secret,
            address: "10.0.0.25/24".parse()?,
            listen_port: None,
            mtu: None,
            buffer_size: Some(32768),
        },
        peers: vec![Peer {
            public_key: peer_public_key,
            endpoint: Some("xxxx".parse()?),
            allowed_ips: vec!["10.0.0.0/24".parse()?],
            persistent_keepalive: Some(15),
        }],
    };

    Ok(config.to_interface().await?)
}
