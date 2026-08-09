use aes::Aes128;
use aes::cipher::{BlockEncrypt, KeyInit};
use anyhow::{Result, bail};
use ctr::Ctr128BE;
use ctr::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
// Public PKG decryption keys — the same ones NoNpDrm/pkgj/pkg2zip use, sourced
// from https://wiki.henkaku.xyz/vita/Packages#AES_Keys. `key_type` (read from
// the ext header, see `header.rs`) selects which one applies to a given PKG.
pub const KEY_PSP: [u8; 16] =
    [0x07, 0xf2, 0xc6, 0x82, 0x90, 0xb5, 0x0d, 0x2c, 0x33, 0x81, 0x8d, 0x70, 0x9b, 0x60, 0xe6, 0x2b];
pub const KEY_VITA_2: [u8; 16] =
    [0xe3, 0x1a, 0x70, 0xc9, 0xce, 0x1d, 0xd7, 0x2b, 0xf3, 0xc0, 0x62, 0x29, 0x63, 0xf2, 0xec, 0xcb];
pub const KEY_VITA_3: [u8; 16] =
    [0x42, 0x3a, 0xca, 0x3a, 0x2b, 0xd5, 0x64, 0x9f, 0x96, 0x86, 0xab, 0xad, 0x6f, 0xd8, 0x80, 0x1f];
pub const KEY_VITA_4: [u8; 16] =
    [0xaf, 0x07, 0xfd, 0x59, 0x65, 0x25, 0x27, 0xba, 0xf1, 0x33, 0x89, 0x66, 0x8b, 0x17, 0xd9, 0xea];
// key_type 1 (PSP/PSX) uses the fixed key; 2/3/4 (Vita) ECB-encrypt the IV under the matching key.
pub fn derive_key(key_type: u32, iv: &[u8; 16]) -> Result<[u8; 16]> {
    match key_type {
        1 => Ok(KEY_PSP),
        2 => Ok(ecb_encrypt(&KEY_VITA_2, iv)),
        3 => Ok(ecb_encrypt(&KEY_VITA_3, iv)),
        4 => Ok(ecb_encrypt(&KEY_VITA_4, iv)),
        other => bail!("unsupported pkg key type {other}"),
    }
}
fn ecb_encrypt(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
    let cipher = Aes128::new(key.into());
    let mut out = aes::Block::clone_from_slice(block);
    cipher.encrypt_block(&mut out);
    out.into()
}
// Seekable AES-128-CTR over the PKG's encrypted region; seek() takes a byte offset.
pub struct PkgCipher(Ctr128BE<Aes128>);
impl PkgCipher {
    pub fn new(key: &[u8; 16], iv: &[u8; 16]) -> Self {
        Self(Ctr128BE::<Aes128>::new(key.into(), iv.into()))
    }
    pub fn decrypt_at(&mut self, offset: u64, buf: &mut [u8]) {
        self.0.seek(offset);
        self.0.apply_keystream(buf);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decrypting_in_one_call_matches_decrypting_in_chunks() {
        let key = KEY_PSP;
        let iv = [0x11u8; 16];
        let mut plain = vec![0u8; 4096];
        for (i, byte) in plain.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        let mut cipher = PkgCipher::new(&key, &iv);
        let mut ciphertext = plain.clone();
        cipher.decrypt_at(0, &mut ciphertext);
        let mut cipher = PkgCipher::new(&key, &iv);
        let mut recovered = vec![0u8; ciphertext.len()];
        let chunk_sizes = [7usize, 1, 32, 500, 16, 3, 4037];
        let mut pos = 0usize;
        for &size in &chunk_sizes {
            let end = (pos + size).min(ciphertext.len());
            if pos >= end {
                break;
            }
            let mut buf = ciphertext[pos..end].to_vec();
            cipher.decrypt_at(pos as u64, &mut buf);
            recovered[pos..end].copy_from_slice(&buf);
            pos = end;
        }
        assert_eq!(&recovered[..pos], &plain[..pos]);
    }
    #[test]
    fn vita_key_types_ecb_encrypt_the_iv() {
        let iv = [0u8; 16];
        let key2 = derive_key(2, &iv).unwrap();
        let key3 = derive_key(3, &iv).unwrap();
        let key4 = derive_key(4, &iv).unwrap();
        assert_ne!(key2, key3);
        assert_ne!(key3, key4);
    }
    #[test]
    fn psp_key_type_is_the_fixed_key_verbatim() {
        assert_eq!(derive_key(1, &[0u8; 16]).unwrap(), KEY_PSP);
    }
    #[test]
    fn unknown_key_type_is_rejected() {
        assert!(derive_key(0, &[0u8; 16]).is_err());
        assert!(derive_key(5, &[0u8; 16]).is_err());
    }
}
