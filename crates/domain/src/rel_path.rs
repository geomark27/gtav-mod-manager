//! Installation-relative path value object.

use crate::{Error, Result};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Maximum accepted length, in characters.
const MAX_LEN: usize = 1024;

/// Characters never allowed in a Windows path component.
const FORBIDDEN_CHARS: [char; 7] = ['<', '>', ':', '"', '|', '?', '*'];

/// A path relative to an installation root.
///
/// Stored with `/` separators and the original case. Equality, ordering and hashing are
/// case-insensitive (Windows semantics) and use [`RelPath::key`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

fn reject<T>(path: &str, reason: &'static str) -> Result<T> {
    Err(Error::InvalidRelPath {
        path: path.to_owned(),
        reason,
    })
}

fn is_reserved_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    for prefix in ["COM", "LPT"] {
        if let Some(rest) = upper.strip_prefix(prefix) {
            let mut chars = rest.chars();
            if let (Some(d), None) = (chars.next(), chars.next()) {
                return ('1'..='9').contains(&d);
            }
        }
    }
    false
}

impl RelPath {
    /// Validates and normalizes a relative path (accepts `/` and `\` separators).
    pub fn parse(input: &str) -> Result<Self> {
        if input.is_empty() {
            return reject(input, "path is empty");
        }
        if input.chars().count() > MAX_LEN {
            return reject(input, "path is longer than 1024 characters");
        }
        if input.starts_with("\\\\") {
            return reject(input, "UNC or verbatim path");
        }
        if input.starts_with('/') || input.starts_with('\\') {
            return reject(input, "absolute path");
        }
        let mut chars = input.chars();
        if let (Some(first), Some(':')) = (chars.next(), chars.next())
            && first.is_ascii_alphabetic()
        {
            return reject(input, "drive letter");
        }
        if input.chars().any(|c| (c as u32) < 0x20) {
            return reject(input, "control character");
        }
        let mut out = String::with_capacity(input.len());
        for (i, component) in input.split(['/', '\\']).enumerate() {
            if component.is_empty() {
                return reject(input, "empty component");
            }
            if component == "." || component == ".." {
                return reject(input, "dot component");
            }
            if component.contains(FORBIDDEN_CHARS) {
                return reject(input, "forbidden character");
            }
            if component.ends_with('.') || component.ends_with(' ') {
                return reject(input, "component ends with dot or space");
            }
            if is_reserved_device_name(component) {
                return reject(input, "reserved device name");
            }
            if i > 0 {
                out.push('/');
            }
            out.push_str(component);
        }
        Ok(Self(out))
    }

    /// The normalized path with `/` separators and original case.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Lowercase comparison key (used for uniqueness and joins in the database).
    pub fn key(&self) -> String {
        self.0.to_lowercase()
    }

    /// Iterates over the path components.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// The last component.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// The parent directory, or `None` for a single component.
    pub fn parent(&self) -> Option<RelPath> {
        self.0.rsplit_once('/').map(|(p, _)| RelPath(p.to_owned()))
    }

    /// Appends a validated segment (itself subject to every `parse` rule).
    pub fn join(&self, segment: &str) -> Result<RelPath> {
        let seg = RelPath::parse(segment)?;
        Ok(RelPath(format!("{}/{}", self.0, seg.0)))
    }

    /// Lowercase extension of the file name, without the dot.
    pub fn extension_lower(&self) -> Option<String> {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => Some(ext.to_lowercase()),
            _ => None,
        }
    }

    /// True when `self` is strictly inside `dir` (case-insensitive, component-wise).
    pub fn is_within(&self, dir: &RelPath) -> bool {
        let mut mine = self.components();
        for d in dir.components() {
            match mine.next() {
                Some(m) if m.to_lowercase() == d.to_lowercase() => {}
                _ => return false,
            }
        }
        mine.next().is_some()
    }
}

impl PartialEq for RelPath {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for RelPath {}

impl PartialOrd for RelPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key().cmp(&other.key())
    }
}

impl Hash for RelPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for RelPath {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl TryFrom<String> for RelPath {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(&value)
    }
}

impl From<RelPath> for String {
    fn from(value: RelPath) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason(input: &str) -> &'static str {
        match RelPath::parse(input) {
            Err(Error::InvalidRelPath { reason, .. }) => reason,
            other => unreachable!("expected rejection for {input:?}, got {other:?}"),
        }
    }

    fn p(s: &str) -> RelPath {
        RelPath::parse(s).unwrap()
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(reason(""), "path is empty");
    }

    #[test]
    fn rejects_absolute() {
        assert_eq!(reason("/x"), "absolute path");
        assert_eq!(reason("\\x"), "absolute path");
    }

    #[test]
    fn rejects_drive_letter() {
        assert_eq!(reason("C:"), "drive letter");
        assert_eq!(reason("C:\\x"), "drive letter");
        assert_eq!(reason("d:/x"), "drive letter");
    }

    #[test]
    fn rejects_unc_and_verbatim() {
        assert_eq!(reason("\\\\server\\share"), "UNC or verbatim path");
        assert_eq!(reason("\\\\?\\C:\\x"), "UNC or verbatim path");
    }

    #[test]
    fn rejects_dot_components() {
        assert_eq!(reason("."), "dot component");
        assert_eq!(reason("a/./b"), "dot component");
        assert_eq!(reason("a/../b"), "dot component");
        assert_eq!(reason(".."), "dot component");
    }

    #[test]
    fn rejects_empty_components() {
        assert_eq!(reason("a//b"), "empty component");
        assert_eq!(reason("a/"), "empty component");
        assert_eq!(reason("a\\"), "empty component");
    }

    #[test]
    fn rejects_control_chars() {
        assert_eq!(reason("a\0b"), "control character");
        assert_eq!(reason("a\tb"), "control character");
        assert_eq!(reason("a\u{1f}b"), "control character");
    }

    #[test]
    fn rejects_forbidden_chars() {
        for c in ['<', '>', ':', '"', '|', '?', '*'] {
            assert_eq!(reason(&format!("a/b{c}c")), "forbidden character", "{c}");
        }
    }

    #[test]
    fn rejects_trailing_dot_or_space() {
        assert_eq!(reason("a/b."), "component ends with dot or space");
        assert_eq!(reason("a/b /c"), "component ends with dot or space");
    }

    #[test]
    fn rejects_reserved_names() {
        for name in [
            "CON", "prn", "Aux", "NUL", "nul.txt", "Com1.ini", "COM9", "lpt1", "LPT9.x",
        ] {
            assert_eq!(reason(name), "reserved device name", "{name}");
            assert_eq!(
                reason(&format!("dir/{name}")),
                "reserved device name",
                "{name}"
            );
        }
    }

    #[test]
    fn accepts_near_reserved_names() {
        for name in [
            "COM0",
            "COM10",
            "LPT",
            "CONSOLE",
            "console.txt",
            "nul_x",
            "aux2",
        ] {
            assert!(RelPath::parse(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_too_long() {
        let long = "a".repeat(1025);
        assert_eq!(reason(&long), "path is longer than 1024 characters");
        assert!(RelPath::parse(&"a".repeat(1024)).is_ok());
    }

    #[test]
    fn normalizes_separators_and_keeps_case() {
        let r = p("Scripts\\Sub/File.ASI");
        assert_eq!(r.as_str(), "Scripts/Sub/File.ASI");
        assert_eq!(r.to_string(), "Scripts/Sub/File.ASI");
    }

    #[test]
    fn equality_and_key_are_case_insensitive() {
        use std::collections::HashSet;
        let a = p("Scripts/A.dll");
        let b = p("scripts\\a.DLL");
        assert_eq!(a, b);
        assert_eq!(a.key(), "scripts/a.dll");
        let set: HashSet<_> = [a, b].into_iter().collect();
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn components_file_name_parent() {
        let r = p("a/B/c.txt");
        assert_eq!(r.components().collect::<Vec<_>>(), ["a", "B", "c.txt"]);
        assert_eq!(r.file_name(), "c.txt");
        assert_eq!(r.parent(), Some(p("a/B")));
        assert_eq!(p("a").parent(), None);
        assert_eq!(p("a").file_name(), "a");
    }

    #[test]
    fn join_validates() {
        assert_eq!(p("a").join("b.txt").unwrap().as_str(), "a/b.txt");
        assert_eq!(p("a").join("b\\c").unwrap().as_str(), "a/b/c");
        assert!(p("a").join("..").is_err());
        assert!(p("a").join("/x").is_err());
        assert!(p("a").join("").is_err());
        assert!(p("a").join("nul").is_err());
    }

    #[test]
    fn extension_lower_cases() {
        assert_eq!(p("a/B.ASI").extension_lower().as_deref(), Some("asi"));
        assert_eq!(p("a/b.tar.GZ").extension_lower().as_deref(), Some("gz"));
        assert_eq!(p("a/noext").extension_lower(), None);
        assert_eq!(p("a/.hidden").extension_lower(), None);
    }

    #[test]
    fn is_within_is_component_wise_and_case_insensitive() {
        let dir = p("Scripts");
        assert!(p("scripts/a.dll").is_within(&dir));
        assert!(p("SCRIPTS/x/y.dll").is_within(&dir));
        assert!(!p("scriptsx/a").is_within(&dir));
        assert!(!p("scripts").is_within(&dir));
        assert!(!p("other/scripts/a").is_within(&dir));
        assert!(p("a/b/c").is_within(&p("A/B")));
    }

    #[test]
    fn serde_round_trip_and_rejection() {
        let r = p("Scripts/A.dll");
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"Scripts/A.dll\"");
        assert_eq!(
            serde_json::from_str::<RelPath>(&json).unwrap().as_str(),
            "Scripts/A.dll"
        );
        assert!(serde_json::from_str::<RelPath>("\"../x\"").is_err());
        assert!(serde_json::from_str::<RelPath>("\"C:\\\\x\"").is_err());
    }
}
