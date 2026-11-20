//! Minimal case-insensitive glob matcher anchored at the installation root.
//!
//! Syntax: `/`-separated segments; `*` matches any run of characters inside one segment,
//! `?` exactly one character inside one segment; a segment that is exactly `**` matches zero
//! or more whole segments. Neither `*` nor `?` ever crosses a `/`.

use crate::error::{Error, Result};
use gmm_domain::RelPath;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    /// `**`: zero or more whole path segments.
    AnyDepth,
    /// A segment pattern (lowercased) possibly containing `*` and `?`.
    Pattern(Vec<char>),
}

/// A compiled glob pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glob {
    source: String,
    segments: Vec<Segment>,
}

impl Glob {
    /// Compiles a pattern. Rejects empty patterns, empty segments, backslashes, absolute
    /// patterns and `**` used inside a longer segment.
    pub fn new(pattern: &str) -> Result<Self> {
        let bad = |reason: &'static str| Error::Glob {
            pattern: pattern.to_owned(),
            reason,
        };
        if pattern.is_empty() {
            return Err(bad("pattern is empty"));
        }
        if pattern.contains('\\') {
            return Err(bad("use / as separator"));
        }
        let mut segments = Vec::new();
        for seg in pattern.split('/') {
            if seg.is_empty() {
                return Err(bad("empty segment"));
            }
            if seg == "**" {
                if segments.last() != Some(&Segment::AnyDepth) {
                    segments.push(Segment::AnyDepth);
                }
                continue;
            }
            if seg.contains("**") {
                return Err(bad("** must be a whole segment"));
            }
            segments.push(Segment::Pattern(seg.to_lowercase().chars().collect()));
        }
        Ok(Self {
            source: pattern.to_owned(),
            segments,
        })
    }

    /// The pattern as written.
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// True when the pattern names exactly one path (no `*`, `?` or `**`).
    pub fn is_literal(&self) -> bool {
        self.segments.iter().all(|s| match s {
            Segment::AnyDepth => false,
            Segment::Pattern(p) => !p.iter().any(|c| matches!(c, '*' | '?')),
        })
    }

    /// True when `path` matches, anchored at the root and case-insensitively.
    pub fn matches(&self, path: &RelPath) -> bool {
        let lowered = path.as_str().to_lowercase();
        let parts: Vec<&str> = lowered.split('/').collect();
        match_segments(&self.segments, &parts)
    }
}

fn match_segments(pattern: &[Segment], parts: &[&str]) -> bool {
    match pattern.split_first() {
        None => parts.is_empty(),
        Some((Segment::AnyDepth, rest)) => {
            (0..=parts.len()).any(|skip| match_segments(rest, &parts[skip..]))
        }
        Some((Segment::Pattern(p), rest)) => match parts.split_first() {
            Some((first, tail)) => match_segment(p, first) && match_segments(rest, tail),
            None => false,
        },
    }
}

/// Matches one segment with `*` and `?` (iterative, linear backtracking).
fn match_segment(pattern: &[char], text: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(glob: &str, path: &str) -> bool {
        let g = Glob::new(glob).unwrap();
        g.matches(&RelPath::parse(path).unwrap())
    }

    #[test]
    fn anchoring() {
        assert!(m("*.rpf", "common.rpf"));
        assert!(!m("*.rpf", "mods/x64a.rpf"));
        assert!(m("dinput8.dll", "dinput8.dll"));
        assert!(!m("dinput8.dll", "scripts/dinput8.dll"));
        assert!(!m("scripts", "scripts/a.asi"));
    }

    #[test]
    fn star_and_question_do_not_cross_slash() {
        assert!(!m("*", "a/b"));
        assert!(m("*", "a"));
        assert!(!m("a?b", "a/b"));
        assert!(m("a?b", "axb"));
        assert!(!m("a?b", "ab"));
        assert!(m("ScriptHookVDotNet*", "ScriptHookVDotNet2.dll"));
        assert!(m("ScriptHookVDotNet*", "ScriptHookVDotNet"));
        assert!(!m("ScriptHookVDotNet*", "x/ScriptHookVDotNet2.dll"));
        assert!(m("a*b*c", "aXXbYYc"));
        assert!(!m("a*b*c", "aXXbYY"));
        assert!(m("*a", "aaa"));
    }

    #[test]
    fn double_star_matches_zero_or_many_segments() {
        assert!(m("mods/**", "mods/a"));
        assert!(m("mods/**", "mods/a/b/c.rpf"));
        assert!(m("mods/**", "mods"));
        assert!(!m("mods/**", "other/mods/a"));
        assert!(m("a/**/c", "a/c"));
        assert!(m("a/**/c", "a/b/c"));
        assert!(m("a/**/c", "a/b/b/c"));
        assert!(!m("a/**/c", "a/b/d"));
        assert!(m("**/x.dll", "x.dll"));
        assert!(m("**/x.dll", "a/b/x.dll"));
        assert!(m("**", "any/thing"));
        assert!(m("a/**/**", "a/b"));
    }

    #[test]
    fn literal_patterns() {
        assert!(Glob::new("ScriptHookV.dll").unwrap().is_literal());
        assert!(Glob::new("scripts/NativeUI.dll").unwrap().is_literal());
        assert!(!Glob::new("scripts/*.dll").unwrap().is_literal());
        assert!(!Glob::new("a?.asi").unwrap().is_literal());
        assert!(!Glob::new("mods/**").unwrap().is_literal());
    }

    #[test]
    fn case_insensitive() {
        assert!(m("ScriptHookV.dll", "SCRIPTHOOKV.DLL"));
        assert!(m("mods/**", "MODS/Update/x.rpf"));
        assert!(m("*.ASI", "trainer.asi"));
    }

    #[test]
    fn invalid_patterns() {
        for bad in ["", "a//b", "/a", "a/", "a\\b", "a**b", "**a", "a/b**"] {
            assert!(Glob::new(bad).is_err(), "{bad:?}");
        }
        assert_eq!(Glob::new("mods/**").unwrap().as_str(), "mods/**");
    }
}
