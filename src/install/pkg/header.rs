use anyhow::{Result, bail};
pub const MAGIC: u32 = 0x7f50_4b47; // "\x7fPKG"
pub const EXT_MAGIC: u32 = 0x7f65_7874; // "\x7fext"

pub const HEADER_SIZE: usize = 192; 
pub const EXT_HEADER_SIZE: usize = 64; 
pub const PROBE_LEN: usize = HEADER_SIZE + EXT_HEADER_SIZE;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Psx,
    Psp,
    VitaApp,
    VitaDlc,
    VitaPsm,
    Unknown(u32),
}
impl ContentType {
    fn from_raw(value: u32) -> Self {
        match value {
            6 => ContentType::Psx,
            7 | 0xe | 0xf | 0x10 => ContentType::Psp,
            0x15 => ContentType::VitaApp,
            0x16 => ContentType::VitaDlc,
            0x18 | 0x1d => ContentType::VitaPsm,
            other => ContentType::Unknown(other),
        }
    }
}
#[derive(Debug, Clone)]
pub struct PkgHeader {
    pub content_id: String,
    pub title_id: String,
    pub item_count: u32,
    pub total_size: u64,
    pub enc_offset: u64,
    pub enc_size: u64,
    pub iv: [u8; 16],
    pub key_type: u32,
    pub content_type: ContentType,
    pub items_offset: u64,
    pub items_size: u64,
}
fn get_u32be(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}
fn get_u64be(b: &[u8]) -> u64 {
    u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}
pub fn parse(probe: &[u8], mut read_at: impl FnMut(u64, usize) -> Result<Vec<u8>>) -> Result<PkgHeader> {
    if probe.len() < PROBE_LEN {
        bail!("pkg header truncated: got {} bytes, need {PROBE_LEN}", probe.len());
    }
    if get_u32be(&probe[0..4]) != MAGIC {
        bail!("not a pkg file (bad magic)");
    }
    if get_u32be(&probe[HEADER_SIZE..HEADER_SIZE + 4]) != EXT_MAGIC {
        bail!("not a pkg file (bad ext header magic)");
    }
    let meta_offset = get_u32be(&probe[8..12]) as u64;
    let meta_count = get_u32be(&probe[12..16]);
    let item_count = get_u32be(&probe[20..24]);
    let total_size = get_u64be(&probe[24..32]);
    let enc_offset = get_u64be(&probe[32..40]);
    let enc_size = get_u64be(&probe[40..48]);
    let content_id: String = probe[0x30..0x30 + 0x30]
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&probe[0x70..0x80]);
    let key_type = (probe[0xE7] & 7) as u32;
    if item_count == 0 {
        bail!("pkg reports zero items");
    }
    let mut content_type_raw = None;
    let mut items_offset_rel = None;
    let mut items_size = None;
    let mut cursor = meta_offset;
    for _ in 0..meta_count {
        let block = read_at(cursor, 16)?;
        if block.len() < 16 {
            bail!("pkg metadata block truncated");
        }
        let entry_type = get_u32be(&block[0..4]);
        let size = get_u32be(&block[4..8]);
        match entry_type {
            2 => content_type_raw = Some(get_u32be(&block[8..12])),
            13 => {
                items_offset_rel = Some(get_u32be(&block[8..12]) as u64);
                items_size = Some(get_u32be(&block[12..16]) as u64);
            }
            _ => {}
        }
        cursor += 8 + size as u64;
    }
    let content_type = ContentType::from_raw(content_type_raw.unwrap_or(0));
    let items_offset_rel = items_offset_rel.unwrap_or(0);
    let expected_items_size = item_count as u64 * 32;
    let items_size = items_size.unwrap_or(expected_items_size);
    if items_size != expected_items_size {
        bail!(
            "pkg item table size mismatch: {item_count} items implies {expected_items_size} bytes, header says {items_size}"
        );
    }
    let title_id = content_id.get(7..16).unwrap_or_default().to_owned();
    Ok(PkgHeader {
        content_id,
        title_id,
        item_count,
        total_size,
        enc_offset,
        enc_size,
        iv,
        key_type,
        content_type,
        items_offset: enc_offset + items_offset_rel,
        items_size,
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    File,
    Directory,
}
#[derive(Debug, Clone)]
pub struct PkgItem {
    pub name_offset: u64,
    pub name_size: u32,
    pub data_offset: u64,
    pub data_size: u64,
    #[allow(dead_code)]
    pub psp_type: u8,
    pub kind: ItemKind,
}
pub fn parse_items(table: &[u8], count: u32) -> Result<Vec<PkgItem>> {
    let mut items = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let entry = table.get(index * 32..index * 32 + 32).ok_or_else(|| anyhow::anyhow!("item table truncated"))?;
        let flags = entry[27];
        items.push(PkgItem {
            name_offset: get_u32be(&entry[0..4]) as u64,
            name_size: get_u32be(&entry[4..8]),
            data_offset: get_u64be(&entry[8..16]),
            data_size: get_u64be(&entry[16..24]),
            psp_type: entry[24],
            kind: if flags == 4 || flags == 18 { ItemKind::Directory } else { ItemKind::File },
        });
    }
    Ok(items)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn u32be(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }
    fn u64be(v: u64) -> [u8; 8] {
        v.to_be_bytes()
    }
    fn synthetic_probe(item_count: u32, items_size: u32, enc_offset: u64, meta_offset: u32) -> Vec<u8> {
        let mut buf = vec![0u8; PROBE_LEN];
        buf[0..4].copy_from_slice(&u32be(MAGIC));
        buf[8..12].copy_from_slice(&u32be(meta_offset));
        buf[12..16].copy_from_slice(&u32be(1)); 
        buf[20..24].copy_from_slice(&u32be(item_count));
        buf[24..32].copy_from_slice(&u64be(enc_offset + items_size as u64 + 1000));
        buf[32..40].copy_from_slice(&u64be(enc_offset));
        buf[40..48].copy_from_slice(&u64be(1000));
        let content_id = b"UP4459-PCSE00487_00-GRAVITYBADGERSHD";
        buf[0x30..0x30 + content_id.len()].copy_from_slice(content_id);
        buf[HEADER_SIZE..HEADER_SIZE + 4].copy_from_slice(&u32be(EXT_MAGIC));
        buf[0xE7] = 2; 
        buf
    }
    #[test]
    fn parses_a_well_formed_header() {
        let item_count = 3;
        let items_size = item_count * 32;
        let enc_offset = 0x100;
        let meta_offset = 0x50;
        let probe = synthetic_probe(item_count, items_size, enc_offset, meta_offset);
        let meta_block = {
            let mut b = Vec::new();
            b.extend_from_slice(&u32be(13));
            b.extend_from_slice(&u32be(8));
            b.extend_from_slice(&u32be(0)); 
            b.extend_from_slice(&u32be(items_size));
            b
        };
        let header = parse(&probe, |offset, len| {
            assert_eq!(offset, meta_offset as u64);
            Ok(meta_block[..len].to_vec())
        })
        .unwrap();
        assert_eq!(header.item_count, 3);
        assert_eq!(header.key_type, 2);
        assert_eq!(header.enc_offset, enc_offset);
        assert_eq!(header.items_offset, enc_offset);
        assert_eq!(header.items_size, items_size as u64);
        assert_eq!(header.title_id, "PCSE00487");
        assert_eq!(header.content_id, "UP4459-PCSE00487_00-GRAVITYBADGERSHD");
    }
    #[test]
    fn rejects_bad_magic() {
        let mut probe = synthetic_probe(1, 32, 0x100, 0x50);
        probe[0] = 0;
        let err = parse(&probe, |_, len| Ok(vec![0u8; len])).unwrap_err();
        assert!(err.to_string().contains("magic"));
    }
    #[test]
    fn rejects_truncated_probe() {
        let err = parse(&[0u8; 10], |_, len| Ok(vec![0u8; len])).unwrap_err();
        assert!(err.to_string().contains("truncated"));
    }
    #[test]
    fn parses_header_without_metadata_block_13() {
        let item_count = 3;
        let expected_size = item_count * 32;
        let enc_offset = 0x100;
        let meta_offset = 0x50;
        let probe = synthetic_probe(item_count, expected_size, enc_offset, meta_offset);
        let meta_block = {
            let mut b = Vec::new();
            b.extend_from_slice(&u32be(2));
            b.extend_from_slice(&u32be(8)); 
            b.extend_from_slice(&u32be(0x15)); 
            b.extend_from_slice(&u32be(0)); 
            b
        };
        let header = parse(&probe, |_, _| Ok(meta_block.clone())).unwrap();
        assert_eq!(header.item_count, 3);
        assert_eq!(header.items_size, expected_size as u64);
        assert_eq!(header.items_offset, enc_offset);
    }
    #[test]
    fn rejects_item_table_size_mismatch() {
        let item_count = 3;
        let wrong_items_size = item_count * 32 + 16; 
        let enc_offset = 0x100;
        let meta_offset = 0x50;
        let probe = synthetic_probe(item_count, item_count * 32, enc_offset, meta_offset);
        let meta_block = {
            let mut b = Vec::new();
            b.extend_from_slice(&u32be(13));
            b.extend_from_slice(&u32be(8));
            b.extend_from_slice(&u32be(0));
            b.extend_from_slice(&u32be(wrong_items_size));
            b
        };
        let err = parse(&probe, |_, len| Ok(meta_block[..len].to_vec())).unwrap_err();
        assert!(err.to_string().contains("mismatch"));
    }
    #[test]
    fn parses_item_table_entries() {
        let mut table = vec![0u8; 64];
        table[0..4].copy_from_slice(&u32be(100));
        table[4..8].copy_from_slice(&u32be(10));
        table[8..16].copy_from_slice(&u64be(200));
        table[16..24].copy_from_slice(&u64be(50));
        table[27] = 0; 
        table[32..36].copy_from_slice(&u32be(300));
        table[36..40].copy_from_slice(&u32be(20));
        table[59] = 4; 
        let items = parse_items(&table, 2).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, ItemKind::File);
        assert_eq!(items[0].data_offset, 200);
        assert_eq!(items[0].data_size, 50);
        assert_eq!(items[1].kind, ItemKind::Directory);
        assert_eq!(items[1].name_offset, 300);
    }
    #[test]
    fn rejects_truncated_item_table() {
        let table = vec![0u8; 16]; 
        assert!(parse_items(&table, 1).is_err());
    }
    #[test]
    fn content_type_mapping_matches_pkg2zip() {
        assert_eq!(ContentType::from_raw(6), ContentType::Psx);
        assert_eq!(ContentType::from_raw(7), ContentType::Psp);
        assert_eq!(ContentType::from_raw(0xe), ContentType::Psp);
        assert_eq!(ContentType::from_raw(0x15), ContentType::VitaApp);
        assert_eq!(ContentType::from_raw(0x16), ContentType::VitaDlc);
        assert_eq!(ContentType::from_raw(0x18), ContentType::VitaPsm);
        assert_eq!(ContentType::from_raw(0xff), ContentType::Unknown(0xff));
    }
}
