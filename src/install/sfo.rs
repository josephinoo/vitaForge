
use anyhow::{Context, Result, bail};

const SFO_MAGIC: u32 = 0x4653_5000;

pub struct SfoIds {
    pub title_id: String,
    pub content_id: Option<String>,
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    let slice = bytes.get(offset..offset + 2).context("sfo truncated")?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    let slice = bytes.get(offset..offset + 4).context("sfo truncated")?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn string_at(bytes: &[u8], offset: usize, len: usize) -> String {
    let raw = bytes.get(offset..offset + len).unwrap_or_default();
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

pub fn read_ids(sfo: &[u8]) -> Result<SfoIds> {
    if u32_at(sfo, 0)? != SFO_MAGIC {
        bail!("param.sfo has a bad magic, this doesn't look like a vita package");
    }
    let key_table = u32_at(sfo, 8)? as usize;
    let data_table = u32_at(sfo, 12)? as usize;
    let entries = u32_at(sfo, 16)? as usize;

    let mut title_id = None;
    let mut content_id = None;

    for index in 0..entries {
        let entry = 20 + index * 16;
        let key_offset = u16_at(sfo, entry)? as usize;
        let value_len = u32_at(sfo, entry + 4)? as usize;
        let data_offset = u32_at(sfo, entry + 12)? as usize;

        let key = string_at(sfo, key_table + key_offset, 32);
        match key.as_str() {
            "TITLE_ID" => title_id = Some(string_at(sfo, data_table + data_offset, value_len)),
            "CONTENT_ID" => {
                let value = string_at(sfo, data_table + data_offset, value_len);
                if !value.is_empty() {
                    content_id = Some(value);
                }
            }
            _ => {}
        }
    }

    Ok(SfoIds { title_id: title_id.context("param.sfo has no TITLE_ID")?, content_id })
}
