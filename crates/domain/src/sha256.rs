//! SHA-256 digest value object.

use crate::{Error, Result};
use std::fmt;

/// A SHA-256 digest: 64 lowercase hex characters.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct Sha256(String);

impl Sha256 {
    /// Parses 64 hex characters of any case; stores them lowercase.
    pub fn parse(s: &str) -> Result<Self> {
        if s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(Self(s.to_ascii_lowercase()))
        } else {
            Err(Error::InvalidSha256(s.to_owned()))
        }
    }

    /// Builds a digest from raw bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(64);
        for b in bytes {
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 0x0f)]));
        }
        Self(out)
    }

    /// The lowercase hex string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for Sha256 {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl TryFrom<String> for Sha256 {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(&value)
    }
}

impl From<Sha256> for String {
    fn from(value: Sha256) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn parses_known_vector() {
        assert_eq!(Sha256::parse(EMPTY).unwrap().as_str(), EMPTY);
        assert_eq!(EMPTY.parse::<Sha256>().unwrap().to_string(), EMPTY);
    }

    #[test]
    fn normalizes_uppercase() {
        let upper = EMPTY.to_uppercase();
        assert_eq!(Sha256::parse(&upper).unwrap().as_str(), EMPTY);
        assert_eq!(
            Sha256::parse(&upper).unwrap(),
            Sha256::parse(EMPTY).unwrap()
        );
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(Sha256::parse(&EMPTY[..63]).is_err());
        assert!(Sha256::parse(&format!("{EMPTY}0")).is_err());
        assert!(Sha256::parse("").is_err());
    }

    #[test]
    fn rejects_non_hex() {
        let bad = format!("g{}", &EMPTY[1..]);
        assert!(matches!(Sha256::parse(&bad), Err(Error::InvalidSha256(_))));
    }

    #[test]
    fn from_bytes_encodes_lowercase_hex() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0xab;
        bytes[31] = 0x0f;
        let s = Sha256::from_bytes(bytes);
        assert!(s.as_str().starts_with("ab00"));
        assert!(s.as_str().ends_with("000f"));
        assert_eq!(s.as_str().len(), 64);
    }

    #[test]
    fn serde_validates() {
        let h = Sha256::parse(EMPTY).unwrap();
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, format!("\"{EMPTY}\""));
        assert_eq!(serde_json::from_str::<Sha256>(&json).unwrap(), h);
        assert!(serde_json::from_str::<Sha256>("\"abc\"").is_err());
    }
}
