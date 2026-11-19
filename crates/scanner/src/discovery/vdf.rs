//! Minimal parser for Valve KeyValues text (`libraryfolders.vdf`, `appmanifest_*.acf`).

use crate::error::{Error, Result};

const MAX_DEPTH: usize = 64;

/// A parsed KeyValues value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vdf {
    /// A string value.
    Str(String),
    /// An object: ordered key/value pairs (keys may repeat).
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// Case-insensitive lookup of a key in an object; `None` for strings or missing keys.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(items) => items
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    /// The string, if this is a string value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }

    /// The entries, if this is an object.
    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Obj(items) => items,
            Vdf::Str(_) => &[],
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Str(String),
    Open,
    Close,
}

fn err(line: usize, message: &str) -> Error {
    Error::Vdf {
        line,
        message: message.to_owned(),
    }
}

fn tokenize(input: &str) -> Result<Vec<(Token, usize)>> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    let mut line = 1usize;
    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            '{' => out.push((Token::Open, line)),
            '}' => out.push((Token::Close, line)),
            '/' => {
                if chars.peek() == Some(&'/') {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            line += 1;
                            break;
                        }
                    }
                } else {
                    return Err(err(line, "unexpected '/'"));
                }
            }
            '"' => {
                let start = line;
                let mut s = String::new();
                loop {
                    match chars.next() {
                        None => return Err(err(start, "unterminated string")),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            None => return Err(err(start, "unterminated string")),
                            Some('\\') => s.push('\\'),
                            Some('"') => s.push('"'),
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => {
                                s.push('\\');
                                s.push(other);
                            }
                        },
                        Some(ch) => {
                            if ch == '\n' {
                                line += 1;
                            }
                            s.push(ch);
                        }
                    }
                }
                out.push((Token::Str(s), start));
            }
            other => return Err(err(line, &format!("unexpected character {other:?}"))),
        }
    }
    Ok(out)
}

fn parse_object(
    tokens: &[(Token, usize)],
    pos: &mut usize,
    depth: usize,
    top: bool,
) -> Result<Vec<(String, Vdf)>> {
    if depth > MAX_DEPTH {
        return Err(err(0, "nesting too deep"));
    }
    let mut items = Vec::new();
    loop {
        match tokens.get(*pos) {
            None => {
                return if top {
                    Ok(items)
                } else {
                    Err(err(tokens.last().map_or(0, |t| t.1), "missing '}'"))
                };
            }
            Some((Token::Close, line)) => {
                if top {
                    return Err(err(*line, "unmatched '}'"));
                }
                *pos += 1;
                return Ok(items);
            }
            Some((Token::Open, line)) => return Err(err(*line, "unexpected '{'")),
            Some((Token::Str(key), line)) => {
                *pos += 1;
                match tokens.get(*pos) {
                    Some((Token::Str(v), _)) => {
                        *pos += 1;
                        items.push((key.clone(), Vdf::Str(v.clone())));
                    }
                    Some((Token::Open, _)) => {
                        *pos += 1;
                        let sub = parse_object(tokens, pos, depth + 1, false)?;
                        items.push((key.clone(), Vdf::Obj(sub)));
                    }
                    _ => return Err(err(*line, "key without value")),
                }
            }
        }
    }
}

/// Parses a KeyValues document into a top-level [`Vdf::Obj`]. Never panics.
pub fn parse(input: &str) -> Result<Vdf> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    Ok(Vdf::Obj(parse_object(&tokens, &mut pos, 0, true)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY_FOLDERS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"228980"		"123456"
		}
	}
	// second library
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"apps"
		{
			"271590"		"98765432"
		}
	}
}
"#;

    #[test]
    fn parses_library_folders() {
        let v = parse(LIBRARY_FOLDERS).unwrap();
        let lf = v.get("LibraryFolders").unwrap();
        assert_eq!(lf.entries().len(), 2);
        assert_eq!(
            lf.get("0")
                .and_then(|l| l.get("path"))
                .and_then(Vdf::as_str),
            Some("C:\\Program Files (x86)\\Steam")
        );
        assert_eq!(
            lf.get("1")
                .and_then(|l| l.get("PATH"))
                .and_then(Vdf::as_str),
            Some("D:\\SteamLibrary")
        );
        assert_eq!(
            lf.get("1")
                .and_then(|l| l.get("apps"))
                .and_then(|a| a.get("271590"))
                .and_then(Vdf::as_str),
            Some("98765432")
        );
    }

    #[test]
    fn parses_appmanifest() {
        let v = parse(
            "\"AppState\"\n{\n\t\"appid\"\t\"271590\"\n\t\"installdir\"\t\"Grand Theft Auto V\"\n}\n",
        )
        .unwrap();
        assert_eq!(
            v.get("appstate")
                .and_then(|s| s.get("installdir"))
                .and_then(Vdf::as_str),
            Some("Grand Theft Auto V")
        );
    }

    #[test]
    fn escapes_and_comments() {
        let v = parse("// header\n\"k\" \"a \\\"q\\\" \\\\ b\" // trailing\n\"n\" {}").unwrap();
        assert_eq!(v.get("k").and_then(Vdf::as_str), Some("a \"q\" \\ b"));
        assert_eq!(v.get("n"), Some(&Vdf::Obj(vec![])));
        assert_eq!(v.get("missing"), None);
    }

    #[test]
    fn empty_input_is_empty_object() {
        assert_eq!(parse("  \n").unwrap(), Vdf::Obj(vec![]));
    }

    #[test]
    fn malformed_input_is_an_error() {
        for bad in [
            "\"a\"",
            "\"a\" {",
            "\"a\" { \"b\" \"c\" ",
            "\"unterminated",
            "}",
            "\"a\" \"b\" }",
            "{ \"a\" \"b\" }",
            "bare words",
            "\"a\" /x",
            "\"a\" \"\\",
        ] {
            assert!(parse(bad).is_err(), "should fail: {bad:?}");
        }
    }

    #[test]
    fn deep_nesting_is_rejected_without_overflow() {
        let mut s = String::new();
        for _ in 0..10_000 {
            s.push_str("\"a\" {");
        }
        assert!(parse(&s).is_err());
    }
}
