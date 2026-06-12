//! `.rkwb` — RohKai Widget Bundle
//!
//! A bundle packages one or more `.rkwd` descriptors into a single portable
//! file.  Two representations are supported:
//!
//! 1. **JSON envelope** — the original format:
//!    ```json
//!    { "format": "rkwb", "schema_version": 1, "descriptors": [ ... ] }
//!    ```
//!
//! 2. **ZIP store-only** — produced by [`build_bundle`].  Each `.rkwd` is
//!    stored as an uncompressed ZIP entry (method=0) plus a `manifest.json`
//!    that lists every entry.  No external crate is used; the ZIP layout is
//!    hand-written using [`std::io`] following the ZIP specification.
//!
//! The ZIP magic bytes `PK\x03\x04` at offset 0 allow any standard ZIP tool
//! to inspect the bundle without RohKai being installed.

use super::widget_descriptor::WidgetDescriptor;
use serde::{Deserialize, Serialize};
use std::io::{Seek, Write};
use std::path::Path;

// ---------------------------------------------------------------------------
// JSON-envelope format (existing)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetBundle {
    pub format: String,
    pub schema_version: u32,
    pub descriptors: Vec<WidgetDescriptor>,
}

impl WidgetBundle {
    pub fn from_descriptors(descriptors: &[WidgetDescriptor]) -> Self {
        Self {
            format: "rkwb".to_owned(),
            schema_version: 1,
            descriptors: descriptors.to_vec(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> Result<Self, BundleError> {
        let b: Self = serde_json::from_str(s).map_err(BundleError::Json)?;
        if b.format != "rkwb" {
            return Err(BundleError::WrongFormat(b.format.clone()));
        }
        if b.schema_version != 1 {
            return Err(BundleError::UnsupportedVersion(b.schema_version));
        }
        Ok(b)
    }

    /// Write each descriptor as `<id>.rkwd` inside `widgets_dir`.
    /// Returns the list of written file stems (descriptor IDs).
    pub fn extract_to(&self, widgets_dir: &Path) -> Result<Vec<String>, std::io::Error> {
        std::fs::create_dir_all(widgets_dir)?;
        let mut written = Vec::new();
        for d in &self.descriptors {
            let json = serde_json::to_string_pretty(d).map_err(std::io::Error::other)?;
            let dest = widgets_dir.join(format!("{}.rkwd", d.id));
            std::fs::write(&dest, json)?;
            written.push(d.id.clone());
        }
        Ok(written)
    }
}

// ---------------------------------------------------------------------------
// ZIP store-only bundle (P2.5)
// ---------------------------------------------------------------------------

/// Write a `.rkwb` ZIP bundle (store-only, no compression) to `path`.
///
/// Used by the bundle installer UI (P2.5) and the future template marketplace.
#[allow(dead_code)] // P2.5: public API, not yet wired to installer UI
///
/// Each descriptor is stored as `<id>.rkwd` plus a top-level `manifest.json`
/// that records every entry.  No external crate is used — the ZIP structure
/// is produced entirely with `std::io`.
///
/// Format: ZIP local-file-header + raw data for each entry, then the central
/// directory and end-of-central-directory record.  Method byte = 0 (STORED).
pub fn build_bundle(descriptors: &[WidgetDescriptor], path: &Path) -> Result<(), String> {
    use std::io::Cursor;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dirs: {e}"))?;
    }

    // Serialise each descriptor to JSON bytes.
    let mut entries: Vec<(String, Vec<u8>)> = descriptors
        .iter()
        .map(|d| {
            let json =
                serde_json::to_string_pretty(d).map_err(|e| format!("serialize {}: {e}", d.id))?;
            Ok((format!("{}.rkwd", d.id), json.into_bytes()))
        })
        .collect::<Result<_, String>>()?;

    // Build the manifest JSON.
    let manifest_names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    let manifest_json = serde_json::json!({
        "format": "rkwb",
        "schema_version": 1,
        "entries": manifest_names
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest_json)
        .map_err(|e| format!("serialize manifest: {e}"))?;
    entries.push(("manifest.json".to_owned(), manifest_bytes));

    // --- Write ZIP to an in-memory buffer then atomically write to disk ---
    let mut buf: Vec<u8> = Vec::new();

    {
        let mut cursor = Cursor::new(&mut buf);

        // Central-directory entries accumulate offsets + metadata.
        struct CdEntry {
            name_bytes: Vec<u8>,
            crc32: u32,
            data_len: u32,
            local_header_offset: u32,
        }
        let mut central: Vec<CdEntry> = Vec::new();

        for (name, data) in &entries {
            let name_bytes = name.as_bytes().to_vec();
            let data_len = data.len() as u32;
            let crc = crc32_ieee(data);
            let local_offset = cursor
                .stream_position()
                .map_err(|e| format!("seek: {e}"))? as u32;

            // Local file header (30 bytes + filename)
            write_local_header(&mut cursor, &name_bytes, data_len, crc)
                .map_err(|e| format!("write local header: {e}"))?;

            cursor
                .write_all(data)
                .map_err(|e| format!("write data: {e}"))?;

            central.push(CdEntry {
                name_bytes,
                crc32: crc,
                data_len,
                local_header_offset: local_offset,
            });
        }

        // Central directory
        let cd_offset = cursor
            .stream_position()
            .map_err(|e| format!("seek cd: {e}"))? as u32;
        for entry in &central {
            write_central_header(
                &mut cursor,
                &entry.name_bytes,
                entry.data_len,
                entry.crc32,
                entry.local_header_offset,
            )
            .map_err(|e| format!("write central dir: {e}"))?;
        }
        let cd_end = cursor
            .stream_position()
            .map_err(|e| format!("seek cd_end: {e}"))? as u32;
        let cd_size = cd_end - cd_offset;

        // End of central directory record (22 bytes)
        write_eocd(
            &mut cursor,
            central.len() as u16,
            cd_size,
            cd_offset,
        )
        .map_err(|e| format!("write eocd: {e}"))?;
    } // cursor dropped here, releasing the mutable borrow on buf

    std::fs::write(path, &buf).map_err(|e| format!("write file: {e}"))
}

// ---------------------------------------------------------------------------
// ZIP low-level helpers — no external crates, std::io only
// ---------------------------------------------------------------------------

/// CRC-32 (IEEE polynomial) over `data`.
#[allow(dead_code)]
fn crc32_ieee(data: &[u8]) -> u32 {
    // Pre-computed CRC-32 table (IEEE 0xEDB88320).
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, cell) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                if c & 1 != 0 {
                    c = 0xEDB88320 ^ (c >> 1);
                } else {
                    c >>= 1;
                }
            }
            *cell = c;
        }
        t
    });

    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Write a ZIP local-file header for an uncompressed entry.
#[allow(dead_code)]
fn write_local_header(
    w: &mut impl Write,
    name: &[u8],
    data_len: u32,
    crc: u32,
) -> std::io::Result<()> {
    w.write_all(b"PK\x03\x04")?; // signature
    w.write_all(&20u16.to_le_bytes())?; // version needed (2.0)
    w.write_all(&0u16.to_le_bytes())?; // general purpose bit flag
    w.write_all(&0u16.to_le_bytes())?; // compression method: STORED
    w.write_all(&0u16.to_le_bytes())?; // last mod time
    w.write_all(&0u16.to_le_bytes())?; // last mod date
    w.write_all(&crc.to_le_bytes())?; // CRC-32
    w.write_all(&data_len.to_le_bytes())?; // compressed size
    w.write_all(&data_len.to_le_bytes())?; // uncompressed size
    w.write_all(&(name.len() as u16).to_le_bytes())?; // file name length
    w.write_all(&0u16.to_le_bytes())?; // extra field length
    w.write_all(name)?; // file name
    Ok(())
}

/// Write a ZIP central-directory entry for an uncompressed file.
#[allow(dead_code)]
fn write_central_header(
    w: &mut impl Write,
    name: &[u8],
    data_len: u32,
    crc: u32,
    local_offset: u32,
) -> std::io::Result<()> {
    w.write_all(b"PK\x01\x02")?; // signature
    w.write_all(&20u16.to_le_bytes())?; // version made by
    w.write_all(&20u16.to_le_bytes())?; // version needed
    w.write_all(&0u16.to_le_bytes())?; // general purpose bit flag
    w.write_all(&0u16.to_le_bytes())?; // compression method: STORED
    w.write_all(&0u16.to_le_bytes())?; // last mod time
    w.write_all(&0u16.to_le_bytes())?; // last mod date
    w.write_all(&crc.to_le_bytes())?; // CRC-32
    w.write_all(&data_len.to_le_bytes())?; // compressed size
    w.write_all(&data_len.to_le_bytes())?; // uncompressed size
    w.write_all(&(name.len() as u16).to_le_bytes())?; // file name length
    w.write_all(&0u16.to_le_bytes())?; // extra field length
    w.write_all(&0u16.to_le_bytes())?; // comment length
    w.write_all(&0u16.to_le_bytes())?; // disk number start
    w.write_all(&0u16.to_le_bytes())?; // internal attributes
    w.write_all(&0u32.to_le_bytes())?; // external attributes
    w.write_all(&local_offset.to_le_bytes())?; // relative offset of local header
    w.write_all(name)?; // file name
    Ok(())
}

/// Write the ZIP end-of-central-directory record.
#[allow(dead_code)]
fn write_eocd(
    w: &mut impl Write,
    num_entries: u16,
    cd_size: u32,
    cd_offset: u32,
) -> std::io::Result<()> {
    w.write_all(b"PK\x05\x06")?; // signature
    w.write_all(&0u16.to_le_bytes())?; // disk number
    w.write_all(&0u16.to_le_bytes())?; // disk with central dir
    w.write_all(&num_entries.to_le_bytes())?; // entries on this disk
    w.write_all(&num_entries.to_le_bytes())?; // total entries
    w.write_all(&cd_size.to_le_bytes())?; // size of central dir
    w.write_all(&cd_offset.to_le_bytes())?; // offset of central dir
    w.write_all(&0u16.to_le_bytes())?; // comment length
    Ok(())
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BundleError {
    Json(serde_json::Error),
    Io(std::io::Error),
    WrongFormat(String),
    UnsupportedVersion(u32),
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::WrongFormat(s) => write!(f, "Not a .rkwb file (format = \"{s}\")"),
            Self::UnsupportedVersion(v) => {
                write!(f, "Unsupported schema_version {v} (expected 1)")
            }
        }
    }
}

impl From<std::io::Error> for BundleError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::widget_descriptor::{
        CanvasPreviewMode, DescriptorCanvasPreview, DescriptorCodegen, WidgetDescriptor,
    };

    fn make_descriptor(id: &str) -> WidgetDescriptor {
        WidgetDescriptor {
            schema_version: 1,
            id: id.to_owned(),
            name: id.to_owned(),
            category: "Test".to_owned(),
            default_size: [100.0, 32.0],
            accent_color: [52, 211, 153],
            properties: vec![],
            state_fields: vec![],
            codegen: DescriptorCodegen {
                live_preview: String::new(),
                export: String::new(),
                on_click_stub: String::new(),
            },
            canvas_preview: DescriptorCanvasPreview {
                mode: CanvasPreviewMode::LabelBox,
                label_template: String::new(),
            },
            cargo_deps: vec![],
            events: vec![],
        }
    }

    #[test]
    fn bundle_write_produces_zip_magic_bytes() {
        let dir =
            std::env::temp_dir().join(format!("rohkai_bundle_{}", uuid::Uuid::new_v4()));
        let path = dir.join("test.rkwb");
        let descriptors = vec![make_descriptor("test.button"), make_descriptor("test.label")];

        build_bundle(&descriptors, &path).expect("build_bundle must succeed");

        let bytes = std::fs::read(&path).expect("file must exist after build_bundle");
        assert!(
            bytes.starts_with(b"PK\x03\x04"),
            "first 4 bytes must be ZIP local-file-header signature PK\\x03\\x04"
        );

        // Verify we can find both .rkwd filenames in the raw bytes.
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("test.button.rkwd"), "bundle must contain test.button.rkwd entry");
        assert!(text.contains("test.label.rkwd"), "bundle must contain test.label.rkwd entry");
        assert!(text.contains("manifest.json"), "bundle must contain manifest.json entry");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn crc32_known_value() {
        // CRC-32 of b"123456789" is 0xCBF43926 per the standard.
        assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);
    }
}
