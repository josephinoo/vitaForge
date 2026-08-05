pub mod crypto;
pub mod header;

use anyhow::{Context, Result, bail};
use crypto::PkgCipher;
use header::{ContentType, ItemKind, PkgHeader};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// What kind of content a downloaded `.pkg` turned out to be, and therefore
/// which layout it needs on disk. `Unsupported` covers PSP (which needs a
/// KIRK-based EBOOT→ISO conversion this extractor does not implement) and PSM
/// — both fall back to BGDL rather than silently producing a broken install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgKind {
    Vita,
    Psx,
    Unsupported,
}

pub fn classify(content_type: ContentType) -> PkgKind {
    match content_type {
        ContentType::VitaApp | ContentType::VitaDlc => PkgKind::Vita,
        ContentType::Psx => PkgKind::Psx,
        ContentType::Psp | ContentType::VitaPsm | ContentType::Unknown(_) => PkgKind::Unsupported,
    }
}

fn read_at(file: &mut std::fs::File, offset: u64, len: usize) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

/// Extracts a PS Vita PKG (app or DLC) into `stage_dir`, laying it out exactly
/// as `scePromoterUtilityPromotePkg` expects: the decrypted content tree plus
/// `sce_sys/package/{head.bin,tail.bin,stat.bin,work.bin}`.
///
/// `head.bin`/`tail.bin` are raw (still-encrypted) byte ranges copied
/// straight from the downloaded file, not decrypted — this matches
/// pkg2zip's behavior (`pkg2zip.c`: head.bin = `[0, enc_offset+items_size)`,
/// tail.bin = `[enc_offset+enc_size, file_end)`), since the promoter expects
/// the genuine Sony package header/trailer, not a re-encoded one.
pub fn extract_vita(pkg_path: &Path, stage_dir: &Path, fake_license: &[u8]) -> Result<PkgHeader> {
    let mut file = std::fs::File::open(pkg_path).context("couldn't reopen the downloaded pkg")?;
    let file_len = file.metadata()?.len();
    let mut probe = vec![0u8; header::PROBE_LEN];
    file.read_exact(&mut probe)?;
    let hdr = header::parse(&probe, |offset, len| read_at(&mut file, offset, len))?;

    if classify(hdr.content_type) != PkgKind::Vita {
        bail!("not a PS Vita app/dlc pkg (content type {:?})", hdr.content_type);
    }
    if file_len < hdr.total_size {
        bail!("pkg file is shorter than its own header claims");
    }

    let key = crypto::derive_key(hdr.key_type, &hdr.iv)?;
    let mut cipher = PkgCipher::new(&key, &hdr.iv);

    let mut table = read_at(&mut file, hdr.items_offset, hdr.items_size as usize)?;
    cipher.decrypt_at(hdr.items_offset - hdr.enc_offset, &mut table);
    let items = header::parse_items(&table, hdr.item_count)?;

    std::fs::create_dir_all(stage_dir)?;
    let mut sce_sys_package_created = false;

    for item in &items {
        let mut name_bytes = read_at(&mut file, hdr.enc_offset + item.name_offset, item.name_size as usize)?;
        cipher.decrypt_at(item.name_offset, &mut name_bytes);
        let name = String::from_utf8(name_bytes).context("pkg item name is not valid utf-8")?;

        let dest = safe_join(stage_dir, &name)?;

        match item.kind {
            ItemKind::Directory => {
                std::fs::create_dir_all(&dest)?;
                if name == "sce_sys/package" {
                    sce_sys_package_created = true;
                }
            }
            ItemKind::File => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                // `digs.bin` ships pre-encrypted and is renamed/kept as-is —
                // matches pkg2zip, which sets `decrypt = 0` for exactly this file.
                let is_digs = name == "sce_sys/package/digs.bin";
                let dest = if is_digs {
                    if !sce_sys_package_created {
                        std::fs::create_dir_all(stage_dir.join("sce_sys/package"))?;
                        sce_sys_package_created = true;
                    }
                    stage_dir.join("sce_sys/package/body.bin")
                } else {
                    dest
                };
                copy_item_data(&mut file, &mut cipher, &hdr, item, !is_digs, &dest)?;
            }
        }
    }

    if !sce_sys_package_created {
        std::fs::create_dir_all(stage_dir.join("sce_sys/package"))?;
    }

    // Raw (still-encrypted) byte ranges — see the doc comment above.
    copy_raw_range(&mut file, 0, hdr.enc_offset + hdr.items_size, &stage_dir.join("sce_sys/package/head.bin"))?;
    copy_raw_range(
        &mut file,
        hdr.enc_offset + hdr.enc_size,
        file_len - (hdr.enc_offset + hdr.enc_size),
        &stage_dir.join("sce_sys/package/tail.bin"),
    )?;
    std::fs::write(stage_dir.join("sce_sys/package/stat.bin"), vec![0u8; 768])?;
    std::fs::write(stage_dir.join("sce_sys/package/work.bin"), fake_license)?;

    Ok(hdr)
}

/// Extracts a PS1 (PSX) PKG. Only `EBOOT.PBP` and `DOCUMENT.DAT` are pulled
/// out (everything else in a PSX pkg is redundant metadata `pkg2zip` also
/// discards), written flat into `dest_dir` — this is the layout Adrenaline
/// expects under `ux0:pspemu/PSP/GAME/{title_id}/`.
pub fn extract_psx(pkg_path: &Path, dest_dir: &Path) -> Result<PkgHeader> {
    let mut file = std::fs::File::open(pkg_path).context("couldn't reopen the downloaded pkg")?;
    let file_len = file.metadata()?.len();
    let mut probe = vec![0u8; header::PROBE_LEN];
    file.read_exact(&mut probe)?;
    let hdr = header::parse(&probe, |offset, len| read_at(&mut file, offset, len))?;

    if classify(hdr.content_type) != PkgKind::Psx {
        bail!("not a PS1 pkg (content type {:?})", hdr.content_type);
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
        let name = String::from_utf8(name_bytes).context("pkg item name is not valid utf-8")?;

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
        bail!("pkg didn't contain USRDIR/CONTENT/EBOOT.PBP — not a playable PSX package");
    }

    Ok(hdr)
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

fn copy_raw_range(file: &mut std::fs::File, start: u64, len: u64, dest: &Path) -> Result<()> {
    const CHUNK: u64 = 1 << 20;
    let mut out = std::fs::File::create(dest).with_context(|| format!("couldn't write {}", dest.display()))?;
    let mut remaining = len;
    let mut offset = start;
    while remaining > 0 {
        let take = remaining.min(CHUNK);
        let buf = read_at(file, offset, take as usize)?;
        std::io::Write::write_all(&mut out, &buf)?;
        offset += take;
        remaining -= take;
    }
    Ok(())
}

/// Rejects a decrypted item name that tries to escape `root` via `..` or an
/// absolute path — a malformed or hostile pkg should not be able to write
/// outside its own staging directory.
fn safe_join(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    let mut dest = root.to_path_buf();
    for part in name.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            bail!("pkg item name escapes its staging directory: {name}");
        }
        dest.push(part);
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::PkgCipher as Cipher;

    fn u32be(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }
    fn u64be(v: u64) -> [u8; 8] {
        v.to_be_bytes()
    }

    /// Builds a complete, validly-encrypted synthetic PKG on disk: header,
    /// one metadata block, an item table describing one directory and one
    /// file, encrypted names/data, and the standard head/tail padding — then
    /// runs it through `extract_vita` end to end and checks the output tree
    /// byte-for-byte. This is the highest-value test for the whole pipeline:
    /// it never has to trust that the header offsets and the extractor agree
    /// with each other, because both sides of that contract run here.
    #[test]
    fn extracts_a_synthetic_vita_pkg_end_to_end() {
        let key_type = 2u32;
        let iv = [0x22u8; 16];
        let key = crypto::derive_key(key_type, &iv).unwrap();

        let dir_name = b"sce_sys/package".to_vec();
        let file_name = b"eboot.bin".to_vec();
        let file_data = b"HELLO VITA PACKAGE CONTENTS 1234567890".to_vec();

        // Lay out the encrypted region: item table, then names, then data.
        let items_offset_rel = 0u64; // items table starts right at enc_offset
        let item_count = 2u32;
        let items_size = item_count as u64 * 32;
        let names_offset_rel = items_size;
        let data_offset_rel = names_offset_rel + (dir_name.len() + file_name.len()) as u64;

        let mut plain_region = Vec::new();
        // Item 0: directory "sce_sys/package"
        {
            let mut entry = [0u8; 32];
            entry[0..4].copy_from_slice(&u32be(names_offset_rel as u32));
            entry[4..8].copy_from_slice(&u32be(dir_name.len() as u32));
            entry[27] = 4; // directory flag
            plain_region.extend_from_slice(&entry);
        }
        // Item 1: file "eboot.bin"
        {
            let name_off = names_offset_rel + dir_name.len() as u64;
            let mut entry = [0u8; 32];
            entry[0..4].copy_from_slice(&u32be(name_off as u32));
            entry[4..8].copy_from_slice(&u32be(file_name.len() as u32));
            entry[8..16].copy_from_slice(&u64be(data_offset_rel));
            entry[16..24].copy_from_slice(&u64be(file_data.len() as u64));
            entry[27] = 0; // file
            plain_region.extend_from_slice(&entry);
        }
        plain_region.extend_from_slice(&dir_name);
        plain_region.extend_from_slice(&file_name);
        plain_region.extend_from_slice(&file_data);

        let enc_size = plain_region.len() as u64;

        // Encrypt the whole plaintext region in one pass (CTR is symmetric).
        let mut cipher = Cipher::new(&key, &iv);
        let mut ciphertext = plain_region.clone();
        cipher.decrypt_at(0, &mut ciphertext);

        let enc_offset = header::PROBE_LEN as u64 + 0x20; // header + a little metadata
        let tail = b"TAILBYTES".to_vec();

        let mut pkg = vec![0u8; header::PROBE_LEN];
        pkg[0..4].copy_from_slice(&u32be(header::MAGIC));
        let meta_offset = header::PROBE_LEN as u64;
        pkg[8..12].copy_from_slice(&u32be(meta_offset as u32));
        pkg[12..16].copy_from_slice(&u32be(2)); // meta_count
        pkg[20..24].copy_from_slice(&u32be(item_count));
        pkg[32..40].copy_from_slice(&u64be(enc_offset));
        pkg[40..48].copy_from_slice(&u64be(enc_size));
        let content_id = b"UP4459-PCSE00487_00-GRAVITYBADGERSHD";
        pkg[0x30..0x30 + content_id.len()].copy_from_slice(content_id);
        pkg[0x70..0x80].copy_from_slice(&iv);
        pkg[header::HEADER_SIZE..header::HEADER_SIZE + 4].copy_from_slice(&u32be(header::EXT_MAGIC));
        pkg[0xE7] = key_type as u8;

        // Metadata block: type=2 (content type) = 0x15, VITA_APP.
        pkg.extend_from_slice(&u32be(2));
        pkg.extend_from_slice(&u32be(4));
        pkg.extend_from_slice(&u32be(0x15));
        // Metadata block: type=13 (item table locator), payload = [items_offset_rel, items_size]
        pkg.extend_from_slice(&u32be(13));
        pkg.extend_from_slice(&u32be(8));
        pkg.extend_from_slice(&u32be(items_offset_rel as u32));
        pkg.extend_from_slice(&u32be(items_size as u32));

        // Pad up to enc_offset, then the encrypted region, then the tail.
        while (pkg.len() as u64) < enc_offset {
            pkg.push(0);
        }
        pkg.extend_from_slice(&ciphertext);
        pkg.extend_from_slice(&tail);

        let total_size = u64be(pkg.len() as u64);
        pkg[24..32].copy_from_slice(&total_size); // total_size

        let dir = std::env::temp_dir().join(format!("vitaforge_pkgtest_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pkg_path = dir.join("test.pkg");
        std::fs::write(&pkg_path, &pkg).unwrap();
        let stage = dir.join("stage");

        let hdr = extract_vita(&pkg_path, &stage, b"FAKE-LICENSE-BYTES").unwrap();

        assert_eq!(hdr.title_id, "PCSE00487");
        assert!(stage.join("sce_sys/package").is_dir());
        assert_eq!(std::fs::read(stage.join("eboot.bin")).unwrap(), file_data);
        assert_eq!(std::fs::read(stage.join("sce_sys/package/work.bin")).unwrap(), b"FAKE-LICENSE-BYTES");
        assert_eq!(std::fs::read(stage.join("sce_sys/package/stat.bin")).unwrap().len(), 768);
        // head.bin covers [0, enc_offset+items_size) raw; tail.bin covers
        // [enc_offset+enc_size, file_end) raw — check they're not empty and
        // land on the expected byte ranges rather than re-deriving the whole
        // file layout a second time here.
        let head = std::fs::read(stage.join("sce_sys/package/head.bin")).unwrap();
        assert_eq!(head.len() as u64, enc_offset + items_size);
        let tail_bin = std::fs::read(stage.join("sce_sys/package/tail.bin")).unwrap();
        assert_eq!(tail_bin, tail);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_join_rejects_path_traversal() {
        let root = Path::new("/tmp/root");
        assert!(safe_join(root, "../../etc/passwd").is_err());
        assert!(safe_join(root, "a/../../b").is_err());
        assert_eq!(safe_join(root, "a/b/c").unwrap(), root.join("a/b/c"));
    }

    #[test]
    fn classify_maps_content_types_to_supported_kinds() {
        assert_eq!(classify(ContentType::VitaApp), PkgKind::Vita);
        assert_eq!(classify(ContentType::VitaDlc), PkgKind::Vita);
        assert_eq!(classify(ContentType::Psx), PkgKind::Psx);
        assert_eq!(classify(ContentType::Psp), PkgKind::Unsupported);
        assert_eq!(classify(ContentType::VitaPsm), PkgKind::Unsupported);
    }
}
