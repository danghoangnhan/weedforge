//! Domain entities for weedforge.

use super::errors::{DomainError, DomainResult};
use std::fmt;
use std::str::FromStr;

/// A `SeaweedFS` file identifier.
///
/// This is a first-class domain entity, not an opaque string.
/// It provides parsing, validation, and rendering of file IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId {
    volume_id: u32,
    file_key: u64,
    cookie: u32,
}

impl FileId {
    /// Creates a new `FileId` from its components.
    #[must_use]
    pub const fn new(volume_id: u32, file_key: u64, cookie: u32) -> Self {
        Self {
            volume_id,
            file_key,
            cookie,
        }
    }

    /// Returns the volume ID.
    #[must_use]
    pub const fn volume_id(&self) -> u32 {
        self.volume_id
    }

    /// Returns the file key.
    #[must_use]
    pub const fn file_key(&self) -> u64 {
        self.file_key
    }

    /// Returns the cookie value.
    #[must_use]
    pub const fn cookie(&self) -> u32 {
        self.cookie
    }

    /// Parses a file ID from a string.
    ///
    /// The expected format is `{volume_id},{hex}` where `hex` encodes the needle key
    /// (up to 8 bytes) followed by the cookie (always 4 bytes / 8 hex chars), matching
    /// `SeaweedFS`. The cookie is therefore the last 8 hex characters and the needle key
    /// is the remaining prefix — a full 64-bit value, not 32-bit.
    ///
    /// # Errors
    ///
    /// Returns an error if the string format is invalid.
    pub fn parse(s: &str) -> DomainResult<Self> {
        let (volume_str, key_cookie_str) =
            s.split_once(',')
                .ok_or_else(|| DomainError::InvalidFileId {
                    value: s.to_string(),
                    reason: "missing comma separator".to_string(),
                })?;

        if volume_str.is_empty() || !volume_str.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DomainError::InvalidFileId {
                value: s.to_string(),
                reason: "invalid volume ID".to_string(),
            });
        }
        let volume_id = volume_str
            .parse::<u32>()
            .map_err(|e| DomainError::InvalidFileId {
                value: s.to_string(),
                reason: format!("invalid volume ID: {e}"),
            })?;

        // The cookie is the last 8 hex chars; the needle key is the prefix (<= 16 hex
        // chars / 64 bits). `SeaweedFS` rejects a key/cookie string of 8 or fewer chars.
        if key_cookie_str.len() <= 8 || key_cookie_str.len() > 24 {
            return Err(DomainError::InvalidFileId {
                value: s.to_string(),
                reason: "key/cookie hex length out of range".to_string(),
            });
        }
        if !key_cookie_str.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(DomainError::InvalidFileId {
                value: s.to_string(),
                reason: "non-hex character in key/cookie".to_string(),
            });
        }

        let split = key_cookie_str.len() - 8;
        let (key_hex, cookie_hex) = key_cookie_str.split_at(split);

        let file_key =
            u64::from_str_radix(key_hex, 16).map_err(|e| DomainError::InvalidFileId {
                value: s.to_string(),
                reason: format!("invalid needle key: {e}"),
            })?;
        let cookie =
            u32::from_str_radix(cookie_hex, 16).map_err(|e| DomainError::InvalidFileId {
                value: s.to_string(),
                reason: format!("invalid cookie: {e}"),
            })?;

        Ok(Self {
            volume_id,
            file_key,
            cookie,
        })
    }

    /// Renders the file ID as its canonical `SeaweedFS` string form.
    ///
    /// The needle key (8 bytes) and cookie (4 bytes) are encoded big-endian with leading
    /// zero bytes stripped, byte-stable with the server's own representation. Handles the
    /// full 64-bit needle-key range with no truncation.
    #[must_use]
    pub fn render(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";

        let mut buf = [0u8; 12];
        buf[0..8].copy_from_slice(&self.file_key.to_be_bytes());
        buf[8..12].copy_from_slice(&self.cookie.to_be_bytes());

        // Strip leading zero bytes, keeping at least the final byte.
        let mut start = 0;
        while start < buf.len() - 1 && buf[start] == 0 {
            start += 1;
        }

        let mut hex = String::with_capacity((buf.len() - start) * 2);
        for &byte in &buf[start..] {
            hex.push(char::from(HEX[(byte >> 4) as usize]));
            hex.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
        format!("{},{hex}", self.volume_id)
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

impl FromStr for FileId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_parse_canonical_roundtrip() {
        let fid = FileId::parse("3,01637037d6").expect("valid fid");
        assert_eq!(fid.volume_id(), 3);
        assert_eq!(fid.file_key(), 1);
        assert_eq!(fid.cookie(), 0x6370_37d6);
        assert_eq!(fid.render(), "3,01637037d6");
    }

    #[test]
    fn test_large_needle_key_roundtrips() {
        // Needle key > 2^32 — previously rejected on parse and truncated on render.
        let fid = FileId::new(7, 0x1_0000_0001, 0xDEAD_BEEF);
        assert_eq!(fid.render(), "7,0100000001deadbeef");
        let parsed = FileId::parse("7,0100000001deadbeef").expect("valid large fid");
        assert_eq!(parsed, fid);
    }

    #[test]
    fn test_max_needle_key() {
        let fid = FileId::parse("1,ffffffffffffffffdeadbeef").expect("valid max fid");
        assert_eq!(fid.file_key(), u64::MAX);
        assert_eq!(fid.cookie(), 0xDEAD_BEEF);
        assert_eq!(fid.render(), "1,ffffffffffffffffdeadbeef");
    }

    #[test]
    fn test_render_canonicalizes_zero_padding() {
        let fid = FileId::parse("3,0000016300007037").expect("valid");
        assert_eq!(fid.render(), "3,016300007037");
    }

    #[test]
    fn test_parse_rejects_plus_sign() {
        assert!(FileId::parse("3,+123456789").is_err());
        assert!(FileId::parse("+3,12345678ab").is_err());
    }

    #[test]
    fn test_parse_rejects_out_of_range_lengths() {
        assert!(FileId::parse("3,12345678").is_err()); // == 8, too short
        assert!(FileId::parse("3,0000000000000000ffffffffff").is_err()); // > 24
    }

    #[test]
    fn test_new_and_accessors() {
        let fid = FileId::new(3, 0x0163, 0x0070_37d6);
        assert_eq!(fid.volume_id(), 3);
        assert_eq!(fid.file_key(), 0x0163);
        assert_eq!(fid.cookie(), 0x0070_37d6);
    }

    #[test]
    fn test_parse_valid() {
        let fid = FileId::parse("3,0000016300007037").ok();
        assert!(fid.is_some());
    }

    #[test]
    fn test_roundtrip() {
        let original = FileId::new(42, 0x1234, 0xABCD_EF00);
        let rendered = original.render();
        let parsed = FileId::parse(&rendered);
        assert!(parsed.is_ok());
        assert_eq!(parsed.ok(), Some(original));
    }
}
