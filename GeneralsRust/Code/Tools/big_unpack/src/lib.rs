use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone)]
struct BigEntry {
    offset: u32,
    size: u32,
    name: String,
}

fn read_exact<const N: usize>(f: &mut File) -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

fn parse_big_index(mut f: &mut File) -> Result<Vec<BigEntry>> {
    // Header: magic(4) + archive_size(le u32) + file_count(be u32) + reserved(4)
    let magic = read_exact::<4>(&mut f)?;
    if &magic != b"BIGF" && &magic != b"BIG4" {
        return Err(anyhow!("unsupported BIG magic: {:?}", magic));
    }
    let size_le = u32::from_le_bytes(read_exact::<4>(&mut f)?);
    let count = u32::from_be_bytes(read_exact::<4>(&mut f)?);
    let _reserved = read_exact::<4>(&mut f)?;
    if size_le == 0 || count == 0 || count > 1_000_000 {
        // sanity
        return Err(anyhow!(
            "invalid BIG header: size={} count={}",
            size_le,
            count
        ));
    }
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let offset = u32::from_be_bytes(read_exact::<4>(&mut f)?);
        let size = u32::from_be_bytes(read_exact::<4>(&mut f)?);
        // Read null-terminated filename
        let mut name_bytes: Vec<u8> = Vec::with_capacity(64);
        loop {
            let mut b = [0u8; 1];
            f.read_exact(&mut b)?;
            if b[0] == 0 {
                break;
            }
            name_bytes.push(b[0]);
            if name_bytes.len() > 4096 {
                return Err(anyhow!("filename too long"));
            }
        }
        let name = String::from_utf8_lossy(&name_bytes).to_string();
        entries.push(BigEntry { offset, size, name });
    }
    Ok(entries)
}

pub fn list_entries(big_path: &Path) -> Result<Vec<String>> {
    let mut f = File::open(big_path).with_context(|| format!("open {}", big_path.display()))?;
    let entries = parse_big_index(&mut f)?;
    Ok(entries.into_iter().map(|e| e.name).collect())
}

pub fn extract_entry(big_path: &Path, entry: &str) -> Result<Vec<u8>> {
    let mut f = File::open(big_path).with_context(|| format!("open {}", big_path.display()))?;
    let entries = parse_big_index(&mut f)?;
    // Match case-insensitive on full path or basename
    let target_lower = entry.to_lowercase();
    let target_base = Path::new(entry)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(entry)
        .to_lowercase();
    let mut found = None;
    for e in entries {
        let name_lower = e.name.to_lowercase();
        let base = Path::new(&e.name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&e.name)
            .to_lowercase();
        if name_lower == target_lower || base == target_base {
            found = Some(e);
            break;
        }
    }
    let e = found.ok_or_else(|| anyhow!("entry not found: {}", entry))?;
    f.seek(SeekFrom::Start(e.offset as u64))?;
    if e.size > 200_000_000 {
        return Err(anyhow!("entry too large: {} bytes", e.size));
    }
    let mut buf = vec![0u8; e.size as usize];
    f.read_exact(&mut buf)?;
    Ok(buf)
}
