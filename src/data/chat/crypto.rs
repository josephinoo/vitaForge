//! End-to-end encryption for chat: XChaCha20-Poly1305 per conversation key, and the
//! key sealed to each member with an ephemeral X25519 exchange.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

pub const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

pub fn random<const N: usize>() -> anyhow::Result<[u8; N]> {
    let mut out = [0u8; N];
    #[cfg(target_os = "vita")]
    {
        // The kernel RNG hands out at most 64 bytes per call.
        for chunk in out.chunks_mut(64) {
            let rc = unsafe { vitasdk_sys::sceKernelGetRandomNumber(chunk.as_mut_ptr().cast(), chunk.len() as _) };
            anyhow::ensure!(rc >= 0, "random generator failed (0x{rc:08x})");
        }
    }
    #[cfg(not(target_os = "vita"))]
    std::io::Read::read_exact(&mut std::fs::File::open("/dev/urandom")?, &mut out)?;
    Ok(out)
}

pub fn public_key(secret: &[u8; KEY_LEN]) -> [u8; KEY_LEN] {
    PublicKey::from(&StaticSecret::from(*secret)).to_bytes()
}

/// Short form of a public key that two people can compare out of band.
pub fn fingerprint(public: &[u8]) -> String {
    let digest = Sha256::digest(public);
    digest[..8].chunks(2).map(|p| format!("{:02X}{:02X}", p[0], p[1])).collect::<Vec<_>>().join(" ")
}

fn seal_with(key: &[u8], plaintext: &[u8], aad: &[u8]) -> anyhow::Result<([u8; NONCE_LEN], Vec<u8>)> {
    let nonce = random::<NONCE_LEN>()?;
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| anyhow::anyhow!("bad key length"))?;
    let sealed = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plaintext, aad })
        .map_err(|_| anyhow::anyhow!("encryption failed"))?;
    Ok((nonce, sealed))
}

fn open_with(key: &[u8], nonce: &[u8], sealed: &[u8], aad: &[u8]) -> Option<Vec<u8>> {
    if nonce.len() != NONCE_LEN {
        return None;
    }
    XChaCha20Poly1305::new_from_slice(key)
        .ok()?
        .decrypt(XNonce::from_slice(nonce), Payload { msg: sealed, aad })
        .ok()
}

fn message_aad(conversation: i64, sender: i64, version: i64) -> Vec<u8> {
    format!("{conversation}|{sender}|{version}").into_bytes()
}

pub fn seal_message(
    key: &[u8; KEY_LEN],
    conversation: i64,
    sender: i64,
    version: i64,
    text: &str,
) -> anyhow::Result<([u8; NONCE_LEN], Vec<u8>)> {
    seal_with(key, text.as_bytes(), &message_aad(conversation, sender, version))
}

pub fn open_message(
    key: &[u8; KEY_LEN],
    conversation: i64,
    sender: i64,
    version: i64,
    nonce: &[u8],
    sealed: &[u8],
) -> Option<String> {
    String::from_utf8(open_with(key, nonce, sealed, &message_aad(conversation, sender, version))?).ok()
}

fn wrapping_key(shared: &[u8; KEY_LEN], ephemeral: &[u8], recipient: &[u8]) -> anyhow::Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    Hkdf::<Sha256>::new(Some(b"vitaforge-chat-v1"), shared)
        .expand_multi_info(&[ephemeral, recipient], &mut key)
        .map_err(|_| anyhow::anyhow!("key derivation failed"))?;
    Ok(key)
}

/// `ephemeral public (32) || nonce (24) || sealed key (48)`.
pub fn wrap_key(
    key: &[u8; KEY_LEN],
    recipient: &[u8; KEY_LEN],
    conversation: i64,
    version: i64,
) -> anyhow::Result<Vec<u8>> {
    let ephemeral = random::<KEY_LEN>()?;
    let ephemeral_public = public_key(&ephemeral);
    let shared = StaticSecret::from(ephemeral).diffie_hellman(&PublicKey::from(*recipient));
    anyhow::ensure!(shared.was_contributory(), "invalid public key");
    let kek = wrapping_key(shared.as_bytes(), &ephemeral_public, recipient)?;
    let (nonce, sealed) = seal_with(&kek, key, format!("{conversation}|{version}").as_bytes())?;
    Ok([&ephemeral_public[..], &nonce, &sealed].concat())
}

pub fn unwrap_key(secret: &[u8; KEY_LEN], wrapped: &[u8], conversation: i64, version: i64) -> Option<[u8; KEY_LEN]> {
    if wrapped.len() <= KEY_LEN + NONCE_LEN {
        return None;
    }
    let (ephemeral, rest) = wrapped.split_at(KEY_LEN);
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let ephemeral: [u8; KEY_LEN] = ephemeral.try_into().ok()?;
    let shared = StaticSecret::from(*secret).diffie_hellman(&PublicKey::from(ephemeral));
    if !shared.was_contributory() {
        return None;
    }
    let kek = wrapping_key(shared.as_bytes(), &ephemeral, &public_key(secret)).ok()?;
    open_with(&kek, nonce, sealed, format!("{conversation}|{version}").as_bytes())?.try_into().ok()
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_roundtrip_is_bound_to_its_context() {
        let key = random::<KEY_LEN>().unwrap();
        let (nonce, sealed) = seal_message(&key, 1, 2, 3, "hola ñ").unwrap();
        assert_eq!(open_message(&key, 1, 2, 3, &nonce, &sealed).as_deref(), Some("hola ñ"));
        assert!(open_message(&key, 9, 2, 3, &nonce, &sealed).is_none());
        assert!(open_message(&key, 1, 9, 3, &nonce, &sealed).is_none());
        assert!(open_message(&key, 1, 2, 9, &nonce, &sealed).is_none());
        assert!(open_message(&key, 1, 2, 3, &nonce[..4], &sealed).is_none());
    }

    #[test]
    fn only_the_recipient_unwraps_the_key() {
        let (alice, eve) = (random::<KEY_LEN>().unwrap(), random::<KEY_LEN>().unwrap());
        let key = random::<KEY_LEN>().unwrap();
        let wrapped = wrap_key(&key, &public_key(&alice), 5, 1).unwrap();
        assert_eq!(wrapped.len(), 104);
        assert_eq!(unwrap_key(&alice, &wrapped, 5, 1), Some(key));
        assert!(unwrap_key(&eve, &wrapped, 5, 1).is_none());
        assert!(unwrap_key(&alice, &wrapped, 5, 2).is_none());
        assert!(unwrap_key(&alice, &wrapped[..40], 5, 1).is_none());
        assert!(wrap_key(&key, &[0; KEY_LEN], 5, 1).is_err());
    }

    #[test]
    fn hex_roundtrip_rejects_garbage() {
        assert_eq!(from_hex(&to_hex(&[0, 255, 16])), Some(vec![0, 255, 16]));
        assert!(from_hex("abc").is_none());
        assert!(from_hex("zz").is_none());
        assert!(from_hex("ñ1ñ1").is_none());
    }
}
