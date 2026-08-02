
use super::sfo;
use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::path::Path;

const HEAD_TEMPLATE: &[u8] = include_bytes!("../../assets/head.bin");
const CONTENT_ID_OFFSET: usize = 0x30;
const CONTENT_ID_LEN: usize = 48;

fn fpkg_hmac(data: &[u8]) -> [u8; 16] {
    let first = Sha1::digest(data);

    let mut block = [0u8; 64];
    block[0..8].copy_from_slice(&first[4..12]);
    block[8..16].copy_from_slice(&first[4..12]);
    block[16..20].copy_from_slice(&first[12..16]);
    block[20] = first[16];
    block[21] = first[1];
    block[22] = first[2];
    block[23] = first[3];
    let mirrored = <[u8; 8]>::try_from(&block[16..24]).expect("slice is 8 bytes");
    block[24..32].copy_from_slice(&mirrored);

    let second = Sha1::digest(block);
    let mut hmac = [0u8; 16];
    hmac.copy_from_slice(&second[..16]);
    hmac
}

fn be_u32(bytes: &[u8], offset: usize) -> Result<usize> {
    let slice = bytes.get(offset..offset + 4).context("head template truncated")?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]) as usize)
}

fn write_at(buffer: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    let slot = buffer.get_mut(offset..offset + bytes.len()).context("head.bin write out of range")?;
    slot.copy_from_slice(bytes);
    Ok(())
}

pub fn write(dir: &Path) -> Result<()> {
    let sfo_bytes = std::fs::read(dir.join("sce_sys/param.sfo"))
        .context("package has no sce_sys/param.sfo - not a valid vpk")?;
    let ids = sfo::read_ids(&sfo_bytes)?;

    let mut head = HEAD_TEMPLATE.to_vec();

    let content_id = ids
        .content_id
        .unwrap_or_else(|| format!("EP9000-{}_00-0000000000000000", ids.title_id));
    let mut id_field = [0u8; CONTENT_ID_LEN];
    let source = content_id.as_bytes();
    let copy_len = source.len().min(CONTENT_ID_LEN);
    id_field[..copy_len].copy_from_slice(&source[..copy_len]);
    write_at(&mut head, CONTENT_ID_OFFSET, &id_field)?;

    let header_len = be_u32(&head, 0xD0)?;
    let hmac = fpkg_hmac(&head[..header_len]);
    write_at(&mut head, header_len, &hmac)?;

    let info_offset = be_u32(&head, 0x08)?;
    let info_len = be_u32(&head, 0x10)?;
    let info_out = be_u32(&head, 0xD4)?;
    let info_end = info_offset + info_len.saturating_sub(64);
    let info_slice = head.get(info_offset..info_end).context("head.bin info range out of bounds")?;
    let hmac = fpkg_hmac(info_slice);
    write_at(&mut head, info_out, &hmac)?;

    let total_len = be_u32(&head, 0xE8)?;
    let hmac = fpkg_hmac(&head[..total_len]);
    write_at(&mut head, total_len, &hmac)?;

    let package_dir = dir.join("sce_sys/package");
    std::fs::create_dir_all(&package_dir).context("couldn't create sce_sys/package")?;
    std::fs::write(package_dir.join("head.bin"), &head).context("couldn't write head.bin")?;
    Ok(())
}
