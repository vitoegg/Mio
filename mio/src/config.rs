use std::net::{Ipv6Addr, SocketAddr};

use base64::{Engine, engine::general_purpose::STANDARD};
use shadowsocks::crypto::CipherKind;

/// The config file, as the init script writes it.
#[derive(serde::Deserialize)]
struct File {
    port: u16,
    password: String,
    cipher: String,
}

pub struct Config {
    pub listen: SocketAddr,
    pub password: String,
    pub method: CipherKind,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|err| format!("{path}: {err}"))?;
        let file = serde_json::from_slice(&data).map_err(|err| err.to_string())?;
        Self::parse(file)
    }

    fn parse(file: File) -> Result<Self, String> {
        let method = file
            .cipher
            .parse::<CipherKind>()
            .ok()
            .filter(CipherKind::is_aead_2022)
            .ok_or_else(|| format!("unsupported cipher: {:?}", file.cipher))?;
        // Only the canonical encoding, the one clients use.
        let size = method.key_len();
        match STANDARD.decode(&file.password) {
            Ok(key) if key.len() == size && STANDARD.encode(&key) == file.password => {}
            _ => return Err(format!("the password of {method} must be a {size}-byte key in base64")),
        }
        Ok(Self {
            // All addresses of IPv4 and IPv6.
            listen: SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), file.port),
            password: file.password,
            method,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "tZr561EnTV6LPmRm+ONfCw==";

    fn parse(password: &str, cipher: &str) -> Result<Config, String> {
        Config::parse(File {
            port: 6160,
            password: password.into(),
            cipher: cipher.into(),
        })
    }

    #[test]
    fn validate() {
        let config = parse(KEY, "2022-blake3-aes-128-gcm").unwrap();
        assert_eq!(config.listen, "[::]:6160".parse().unwrap());
        let key = "VtVaE4cpmqL6LQbWolhOfQm2OAUhZ/l1EXKS5wo5+g4=";
        assert!(parse(key, "2022-blake3-chacha20-poly1305").is_ok());
        assert!(parse(key, "2022-blake3-aes-256-gcm").is_ok());

        for (password, cipher) in [
            (KEY, "aes-128-gcm"),
            (KEY, "none"),
            (KEY, "2022-blake3-aes-256-gcm"),
            ("tZr561EnTV6LPmRm+ONfCx==", "2022-blake3-aes-128-gcm"),
            (&format!("{KEY}\n"), "2022-blake3-aes-128-gcm"),
            ("", "2022-blake3-aes-128-gcm"),
        ] {
            assert!(parse(password, cipher).is_err(), "{password} {cipher}");
        }
    }
}
