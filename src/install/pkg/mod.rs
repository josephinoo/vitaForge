pub mod crypto;
pub mod header;
use anyhow::{Context, Result, bail};
use crypto::PkgCipher;
use header::{ContentType, ItemKind, PkgHeader};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgKind {
    Vita,
    Psx,
    Psp,
    Unsupported,
}
pub fn classify(content_type: ContentType) -> PkgKind {
    match content_type {
        ContentType::VitaApp | ContentType::VitaDlc => PkgKind::Vita,
        ContentType::Psx => PkgKind::Psx,
        ContentType::Psp => PkgKind::Psp,
        ContentType::VitaPsm | ContentType::Unknown(_) => PkgKind::Unsupported,
    }
}
fn read_at(file: &mut std::fs::File, offset: u64, len: usize) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

fn extract_pbp_pkg(pkg_path: &Path, dest_dir: &Path, expected: PkgKind) -> Result<PkgHeader> {
    let mut file = std::fs::File::open(pkg_path).context("couldn't reopen the downloaded pkg")?;
    let file_len = file.metadata()?.len();
    let mut probe = vec![0u8; header::PROBE_LEN];
    file.read_exact(&mut probe)?;
    let hdr = header::parse(&probe, |offset, len| read_at(&mut file, offset, len))?;
    if classify(hdr.content_type) != expected {
        bail!("not a {:?} pkg (content type {:?})", expected, hdr.content_type);
    }
    if file_len < hdr.total_size {
        bail!("pkg file is shorter than its own header claims");
    }
    let key = crypto::derive_key(hdr.key_type, &hdr.iv)?;
    let mut cipher = PkgCipher::new(&key, &hdr.iv);
    let mut table = read_at(&mut file, hdr.items_offset, hdr.items_size as usize)?;
    cipher.decrypt_at(hdr.items_offset - hdr.enc_offset, &mut table);
    let items = header::parse_items(&table, hdr.item_count)?;
    std::fs::create_dir_all(dest_dir)?;
    let mut found_eboot = false;
    for item in &items {
        if item.kind != ItemKind::File {
            continue;
        }
        let mut name_bytes = read_at(&mut file, hdr.enc_offset + item.name_offset, item.name_size as usize)?;
        cipher.decrypt_at(item.name_offset, &mut name_bytes);
        // Signature/hash items in the table don't have textual names — skip rather than fail.
        let Ok(name) = String::from_utf8(name_bytes) else { continue };
        let dest = match name.as_str() {
            "USRDIR/CONTENT/EBOOT.PBP" => {
                found_eboot = true;
                dest_dir.join("EBOOT.PBP")
            }
            "USRDIR/CONTENT/DOCUMENT.DAT" => dest_dir.join("DOCUMENT.DAT"),
            _ => continue,
        };
        copy_item_data(&mut file, &mut cipher, &hdr, item, true, &dest)?;
    }
    if !found_eboot {
        bail!("pkg didn't contain USRDIR/CONTENT/EBOOT.PBP");
    }
    Ok(hdr)
}
pub fn extract_psx(pkg_path: &Path, dest_dir: &Path) -> Result<PkgHeader> {
    extract_pbp_pkg(pkg_path, dest_dir, PkgKind::Psx)
}
pub fn extract_psp(pkg_path: &Path, dest_dir: &Path) -> Result<PkgHeader> {
    // Unified path with PS1: both use identical AES decryption + USRDIR/CONTENT/EBOOT.PBP extraction
    // (usagi-pkgj uses same pkgi_install_pspgame for both; PSP-specific logic deferred to disc-ID routing)
    extract_pbp_pkg(pkg_path, dest_dir, PkgKind::Psp)
}
fn copy_item_data(
    file: &mut std::fs::File,
    cipher: &mut PkgCipher,
    hdr: &PkgHeader,
    item: &header::PkgItem,
    decrypt: bool,
    dest: &Path,
) -> Result<()> {
    const CHUNK: u64 = 1 << 20;
    let mut out = std::fs::File::create(dest).with_context(|| format!("couldn't write {}", dest.display()))?;
    let mut remaining = item.data_size;
    let mut offset = item.data_offset;
    while remaining > 0 {
        let take = remaining.min(CHUNK);
        let mut buf = read_at(file, hdr.enc_offset + offset, take as usize)?;
        if decrypt {
            cipher.decrypt_at(offset, &mut buf);
        }
        std::io::Write::write_all(&mut out, &buf)?;
        offset += take;
        remaining -= take;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classify_maps_content_types_to_supported_kinds() {
        assert_eq!(classify(ContentType::VitaApp), PkgKind::Vita);
        assert_eq!(classify(ContentType::VitaDlc), PkgKind::Vita);
        assert_eq!(classify(ContentType::Psx), PkgKind::Psx);
        assert_eq!(classify(ContentType::Psp), PkgKind::Psp);
        assert_eq!(classify(ContentType::VitaPsm), PkgKind::Unsupported);
    }
}
