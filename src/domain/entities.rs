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
    /// The expected format is `{volume_id},{file_key}{cookie}`.
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

        let volume_id = volume_str
            .parse::<u32>()
            .map_err(|e| DomainError::InvalidFileId {
                value: s.to_string(),
                reason: format!("invalid volume ID: {e}"),
            })?;

        if key_cookie_str.len() < 8 {
            return Err(DomainError::InvalidFileId {
                value: s.to_string(),
                reason: "key/cookie hex string too short".to_string(),
            });
        }

        let full_value =
            u64::from_str_radix(key_cookie_str, 16).map_err(|e| DomainError::InvalidFileId {
                value: s.to_string(),
                reason: format!("invalid hex in key/cookie: {e}"),
            })?;

        let cookie = (full_value & 0xFFFF_FFFF) as u32;
        let file_key = full_value >> 32;

        Ok(Self {
            volume_id,
            file_key,
            cookie,
        })
    }

    /// Renders the file ID as a string suitable for URLs.
    #[must_use]
    pub fn render(&self) -> String {
        let combined = (self.file_key << 32) | u64::from(self.cookie);
        format!("{},{:016x}", self.volume_id, combined)
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
    use super::*;

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
