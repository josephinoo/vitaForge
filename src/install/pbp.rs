use std::path::Path;

const PBP_MAGIC: &[u8; 4] = b"\0PBP";

pub fn read_disc_id(pbp_path: &Path) -> Option<String> {
    // Port of usagi-pkgj's pkgi_pbp_read_disc_id (install.cpp)
    let mut file = std::fs::File::open(pbp_path).ok()?;
    use std::io::{Read, Seek, SeekFrom};

    let mut header = [0u8; 32];
    file.read_exact(&mut header).ok()?;

    if &header[0..4] != PBP_MAGIC {
        return None;
    }

    // PBP header: magic + 8 u32-LE offsets (param_sfo, icon0_png, icon1_pmf, pic0_png, pic1_png, snd0_at3, psp_data, psar_data)
    let offset_param_sfo = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
    let offset_icon0_png =
        u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as u64;

    // Minis and other non-standard PBPs can have zeroed/out-of-order offsets; bail instead of
    // underflowing into a multi-terabyte allocation.
    if offset_icon0_png <= offset_param_sfo {
        return None;
    }
    let sfo_len = (offset_icon0_png - offset_param_sfo) as usize;
    const MAX_SFO_LEN: usize = 256 * 1024;
    if sfo_len > MAX_SFO_LEN {
        return None;
    }
    file.seek(SeekFrom::Start(offset_param_sfo)).ok()?;

    let mut sfo_buf = vec![0u8; sfo_len];
    file.read_exact(&mut sfo_buf).ok()?;

    let ids = crate::install::sfo::read_ids(&sfo_buf).ok()?;

    if let Some(disc_id) = ids.disc_id {
        if disc_id.len() >= 9 {
            return Some(disc_id);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sfo_with_disc_id(disc_id: &str) -> Vec<u8> {
        let mut sfo = vec![0u8; 256];

        sfo[0..4].copy_from_slice(&[0x00, 0x50, 0x53, 0x46]);
        sfo[8..12].copy_from_slice(&68u32.to_le_bytes());
        sfo[12..16].copy_from_slice(&150u32.to_le_bytes());
        sfo[16..20].copy_from_slice(&3u32.to_le_bytes());

        // Header (20) + 3 index entries (16 bytes each) = 68, then the key table, then the
        // data table — these regions must not overlap the fixed-size index/header above them.
        let key_table_offset = 68;
        let data_table_offset = 150;

        let title_id = "SLUS12345";
        let content_id = "UP0001-SLUS12345_00-0000000000000000";

        sfo[key_table_offset..key_table_offset + 8].copy_from_slice(b"TITLE_ID");
        sfo[key_table_offset + 32..key_table_offset + 32 + 8].copy_from_slice(b"DISC_ID\0");
        sfo[key_table_offset + 64..key_table_offset + 64 + 10].copy_from_slice(b"CONTENT_ID");

        sfo[20 + 0 * 16..20 + 0 * 16 + 2].copy_from_slice(&0u16.to_le_bytes());
        sfo[20 + 0 * 16 + 4..20 + 0 * 16 + 8]
            .copy_from_slice(&(title_id.len() as u32).to_le_bytes());
        sfo[20 + 0 * 16 + 12..20 + 0 * 16 + 16].copy_from_slice(&0u32.to_le_bytes());

        sfo[20 + 1 * 16..20 + 1 * 16 + 2].copy_from_slice(&32u16.to_le_bytes());
        sfo[20 + 1 * 16 + 4..20 + 1 * 16 + 8]
            .copy_from_slice(&(disc_id.len() as u32).to_le_bytes());
        sfo[20 + 1 * 16 + 12..20 + 1 * 16 + 16].copy_from_slice(&10u32.to_le_bytes());

        sfo[20 + 2 * 16..20 + 2 * 16 + 2].copy_from_slice(&64u16.to_le_bytes());
        sfo[20 + 2 * 16 + 4..20 + 2 * 16 + 8]
            .copy_from_slice(&(content_id.len() as u32).to_le_bytes());
        sfo[20 + 2 * 16 + 12..20 + 2 * 16 + 16].copy_from_slice(&50u32.to_le_bytes());

        sfo[data_table_offset..data_table_offset + title_id.len()]
            .copy_from_slice(title_id.as_bytes());
        sfo[data_table_offset + 10..data_table_offset + 10 + disc_id.len()]
            .copy_from_slice(disc_id.as_bytes());
        sfo[data_table_offset + 50..data_table_offset + 50 + content_id.len()]
            .copy_from_slice(content_id.as_bytes());

        sfo
    }

    #[test]
    fn disc_id_reads_from_synthetic_pbp() {
        use std::io::Write;

        let temp_dir = std::env::temp_dir().join("pbp_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let pbp_path = temp_dir.join("test.pbp");

        let sfo_data = make_sfo_with_disc_id("SLUS12345");
        let mut pbp = Vec::new();
        pbp.extend_from_slice(b"\0PBP");
        pbp.extend_from_slice(&36u32.to_le_bytes());
        pbp.extend_from_slice(&(36 + sfo_data.len() as u32).to_le_bytes());
        pbp.extend_from_slice(&0u32.to_le_bytes());
        pbp.extend_from_slice(&0u32.to_le_bytes());
        pbp.extend_from_slice(&0u32.to_le_bytes());
        pbp.extend_from_slice(&0u32.to_le_bytes());
        pbp.extend_from_slice(&0u32.to_le_bytes());
        pbp.extend_from_slice(&0u32.to_le_bytes());
        pbp.extend_from_slice(&sfo_data);

        let mut file = std::fs::File::create(&pbp_path).unwrap();
        file.write_all(&pbp).unwrap();
        drop(file);

        let result = read_disc_id(&pbp_path);
        let _ = std::fs::remove_file(pbp_path);

        assert_eq!(result, Some("SLUS12345".to_string()));
    }

    #[test]
    fn disc_id_returns_none_on_invalid_magic() {
        use std::io::Write;

        let temp_dir = std::env::temp_dir().join("pbp_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let pbp_path = temp_dir.join("test_bad_magic.pbp");

        let mut pbp = vec![0xFFu8; 40];
        pbp[0..4].copy_from_slice(b"XXXX");

        let mut file = std::fs::File::create(&pbp_path).unwrap();
        file.write_all(&pbp).unwrap();
        drop(file);

        let result = read_disc_id(&pbp_path);
        let _ = std::fs::remove_file(pbp_path);

        assert_eq!(result, None);
    }
}
