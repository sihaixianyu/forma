//! Versioned, self-contained sharing snapshots. Document text stays byte-for-byte intact.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const MAX_LINK_LENGTH: usize = 16_000;
pub const MAX_SNAPSHOT_BYTES: usize = 1_048_576;
pub const WEB_APP: &str = "https://sihaixianyu.github.io/forma/";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub left: String,
    pub right: String,
    pub compare: bool,
}

pub fn link(base: &str, snapshot: &Snapshot) -> Result<String, String> {
    if snapshot.left.len().saturating_add(snapshot.right.len()) > MAX_SNAPSHOT_BYTES {
        return Err("Content is too large for a share link. Export JSON instead.".into());
    }
    let bytes = serde_json::to_vec(snapshot).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("Content is too large for a share link. Export JSON instead.".into());
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&bytes).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    let link = format!(
        "{}#share=v1.{}",
        base.split(['?', '#']).next().unwrap_or(base),
        URL_SAFE_NO_PAD.encode(compressed)
    );
    if link.len() > MAX_LINK_LENGTH {
        return Err("Share link exceeds 16,000 characters. Export JSON instead.".into());
    }
    Ok(link)
}

pub fn from_fragment(fragment: &str) -> Result<Option<Snapshot>, String> {
    let Some(payload) = fragment.strip_prefix("#share=") else {
        return Ok(None);
    };
    if fragment.len() > MAX_LINK_LENGTH {
        return Err("Share link is too long".into());
    }
    let payload = payload
        .strip_prefix("v1.")
        .ok_or("Unsupported share link version")?;
    let compressed = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| "Invalid share link encoding")?;
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut bytes = Vec::new();
    (&mut decoder)
        .take(MAX_SNAPSHOT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Invalid compressed share content")?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("Shared content exceeds the 1 MiB limit".into());
    }
    if decoder.total_in() != compressed.len() as u64 {
        return Err("Invalid compressed share content".into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "Invalid share snapshot".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_text_and_mode() {
        for compare in [false, true] {
            let snapshot = Snapshot {
                left: "{\n  \"中文\": 123456789012345678901234567890, \"b\": 1\n}".into(),
                right: "unfinished {".into(),
                compare,
            };
            let url = link("https://example.com/forma/?debug=1#old", &snapshot).unwrap();
            assert!(url.starts_with("https://example.com/forma/#share=v1."));
            assert_eq!(
                from_fragment(&url[url.find('#').unwrap()..]).unwrap(),
                Some(snapshot)
            );
        }
    }

    #[test]
    fn rejects_bad_links_and_unrelated_fragments() {
        assert_eq!(from_fragment("#other").unwrap(), None);
        for fragment in [
            "#share=v2.abc",
            "#share=v1.!",
            "#share=v1.",
            "#share=v1.YWJj",
        ] {
            assert!(from_fragment(fragment).is_err(), "{fragment}");
        }
    }

    #[test]
    fn bounds_encoded_and_decompressed_content() {
        let snapshot = Snapshot {
            left: "x".repeat(MAX_SNAPSHOT_BYTES + 1),
            right: String::new(),
            compare: false,
        };
        assert!(link(WEB_APP, &snapshot).is_err());
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(snapshot.left.as_bytes()).unwrap();
        let fragment = format!(
            "#share=v1.{}",
            URL_SAFE_NO_PAD.encode(encoder.finish().unwrap())
        );
        assert!(from_fragment(&fragment).unwrap_err().contains("1 MiB"));
        assert!(from_fragment(&format!("#share=v1.{}", "a".repeat(MAX_LINK_LENGTH))).is_err());
    }
}
