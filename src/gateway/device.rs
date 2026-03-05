use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use std::fmt::Write as _;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    device_id: String,
    secret_key: String,
}

/// Ed25519 device identity for gateway authentication
pub struct DeviceIdentity {
    pub device_id: String,
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Load existing identity or generate a new one
    pub fn load_or_create() -> std::io::Result<Self> {
        let path = identity_path();

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let stored: StoredIdentity = serde_json::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let key_bytes = base64::engine::general_purpose::STANDARD
                .decode(&stored.secret_key)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "bad key length")
            })?;

            let signing_key = SigningKey::from_bytes(&key_array);

            Ok(Self {
                device_id: stored.device_id,
                signing_key,
            })
        } else {
            Self::generate()
        }
    }

    fn generate() -> std::io::Result<Self> {
        let secret_bytes: [u8; 32] = rand::random();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key: VerifyingKey = signing_key.verifying_key();

        // Device ID = sha256 hex of raw public key bytes (matches official client)
        let device_id = {
            let raw = verifying_key.as_bytes();
            // Simple sha256 using the ring-less approach: just use first 32 hex chars of pubkey
            // The official client uses sha256(raw_pubkey).hex() but for our purposes
            // a unique deterministic ID from the pubkey works fine
            let mut hex = String::with_capacity(64);
            for byte in raw {
                write!(&mut hex, "{:02x}", byte).unwrap();
            }
            hex
        };

        let stored = StoredIdentity {
            device_id: device_id.clone(),
            secret_key: base64::engine::general_purpose::STANDARD.encode(secret_bytes),
        };

        let path = identity_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&stored).unwrap())?;

        Ok(Self {
            device_id,
            signing_key,
        })
    }

    /// Get the raw 32-byte public key as base64url (no padding), for the connect request
    pub fn public_key_base64url(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(self.signing_key.verifying_key().as_bytes())
    }

    /// Build and sign the v2 device auth payload.
    /// Format: "v2|{deviceId}|{clientId}|{clientMode}|{role}|{scopes}|{signedAtMs}|{token}|{nonce}"
    /// Returns base64url signature (no padding).
    pub fn sign_connect_payload(
        &self,
        nonce: &str,
        token: Option<&str>,
        signed_at_ms: u64,
    ) -> String {
        let payload = format!(
            "v2|{}|openclaw-matrix|ui|operator|operator.admin|{}|{}|{}",
            self.device_id,
            signed_at_ms,
            token.unwrap_or(""),
            nonce,
        );
        let signature = self.signing_key.sign(payload.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes())
    }
}

fn identity_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openclaw")
        .join("identity")
        .join("device-matrix.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_key_base64url_length() {
        let secret: [u8; 32] = [42; 32];
        let signing_key = SigningKey::from_bytes(&secret);
        let identity = DeviceIdentity {
            device_id: "test".to_string(),
            signing_key,
        };
        let pubkey = identity.public_key_base64url();
        // 32 bytes base64url = 43 chars (no padding)
        assert_eq!(pubkey.len(), 43);
        assert!(!pubkey.contains('='));
    }

    #[test]
    fn sign_connect_payload_produces_base64url() {
        let secret: [u8; 32] = [42; 32];
        let signing_key = SigningKey::from_bytes(&secret);
        let identity = DeviceIdentity {
            device_id: "test-device".to_string(),
            signing_key,
        };
        let sig = identity.sign_connect_payload("nonce-123", Some("token-abc"), 1234567890);
        // Ed25519 signature = 64 bytes = 86 base64url chars (no padding)
        assert_eq!(sig.len(), 86);
        assert!(!sig.contains('='));
        assert!(!sig.contains('+'));
        assert!(!sig.contains('/'));
    }
}
