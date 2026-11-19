//! Names of engine artifacts (temp files and tombstones), see architecture section 7.2.

use crate::TransactionId;

const TMP_MARKER: &str = ".gmm-tmp-";
const TOMB_MARKER: &str = ".gmm-tomb-";
const SUFFIX_LEN: usize = 8;

/// Kind of engine artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// Temporary file written before a no-replace rename.
    Tmp,
    /// Tombstone of a game file awaiting commit.
    Tomb,
}

/// A parsed engine artifact name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Artifact {
    /// File name the artifact belongs to.
    pub original: String,
    /// Temp file or tombstone.
    pub kind: ArtifactKind,
    /// Transaction suffix (8 lowercase hex characters).
    pub suffix: String,
}

/// The last 8 hex characters of the transaction UUID (its random part; the leading characters
/// of a v7 UUID are a timestamp and collide between transactions created close together).
pub fn tx_suffix(tx: TransactionId) -> String {
    let simple = tx.as_uuid().simple().to_string();
    simple[simple.len() - SUFFIX_LEN..].to_owned()
}

/// `{file_name}.gmm-tmp-{suffix}`.
pub fn tmp_name(file_name: &str, tx: TransactionId) -> String {
    format!("{file_name}{TMP_MARKER}{}", tx_suffix(tx))
}

/// `{file_name}.gmm-tomb-{suffix}`.
pub fn tomb_name(file_name: &str, tx: TransactionId) -> String {
    format!("{file_name}{TOMB_MARKER}{}", tx_suffix(tx))
}

/// Parses an artifact name; `None` when it is not one (suffix must be 8 lowercase hex chars
/// and the original name non-empty).
pub fn parse_artifact(name: &str) -> Option<Artifact> {
    for (marker, kind) in [
        (TMP_MARKER, ArtifactKind::Tmp),
        (TOMB_MARKER, ArtifactKind::Tomb),
    ] {
        if let Some(idx) = name.rfind(marker) {
            let suffix = &name[idx + marker.len()..];
            let valid = suffix.len() == SUFFIX_LEN
                && suffix
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            if valid && idx > 0 {
                return Some(Artifact {
                    original: name[..idx].to_owned(),
                    kind,
                    suffix: suffix.to_owned(),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn tx(s: &str) -> TransactionId {
        TransactionId::from_uuid(Uuid::parse_str(s).unwrap())
    }

    #[test]
    fn suffix_is_last_eight_hex_chars() {
        let id = tx("01890a5d-ac96-774b-bcce-b302099a8057");
        assert_eq!(tx_suffix(id), "099a8057");
    }

    #[test]
    fn names() {
        let id = tx("01890a5d-ac96-774b-bcce-b302099a8057");
        assert_eq!(tmp_name("a.asi", id), "a.asi.gmm-tmp-099a8057");
        assert_eq!(tomb_name("a.asi", id), "a.asi.gmm-tomb-099a8057");
    }

    #[test]
    fn suffixes_differ_for_back_to_back_ids() {
        let a = TransactionId::new();
        let b = TransactionId::new();
        assert_ne!(tx_suffix(a), tx_suffix(b));
        assert_eq!(tx_suffix(a).len(), 8);
    }

    #[test]
    fn parse_round_trip() {
        let id = TransactionId::new();
        let tmp = parse_artifact(&tmp_name("my.file.dll", id)).unwrap();
        assert_eq!(
            tmp,
            Artifact {
                original: "my.file.dll".into(),
                kind: ArtifactKind::Tmp,
                suffix: tx_suffix(id)
            }
        );
        let tomb = parse_artifact(&tomb_name("x", id)).unwrap();
        assert_eq!(tomb.kind, ArtifactKind::Tomb);
        assert_eq!(tomb.original, "x");
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "a.asi",
            "a.asi.gmm-tmp-",
            "a.asi.gmm-tmp-1234567",
            "a.asi.gmm-tmp-123456789",
            "a.asi.gmm-tmp-ABCDEF12",
            "a.asi.gmm-tmp-1234567g",
            "a.asi.gmm-tomb-xyz",
            ".gmm-tmp-12345678",
            "a.asi.gmm-tmp12345678",
            "",
        ] {
            assert_eq!(parse_artifact(bad), None, "{bad}");
        }
    }
}
