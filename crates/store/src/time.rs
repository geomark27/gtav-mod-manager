//! Timestamp encoding: RFC 3339 UTC with milliseconds.

use chrono::{DateTime, SecondsFormat, Utc};

use crate::error::{Error, Result};

/// Encodes a timestamp for storage, e.g. `2026-11-18T21:06:00.000Z`.
pub(crate) fn to_db(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Decodes a stored timestamp; `Corrupt` when it does not parse.
pub(crate) fn from_db(table: &'static str, column: &'static str, s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| Error::Corrupt {
            table,
            column,
            value: s.to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_format() {
        let t = DateTime::parse_from_rfc3339("2026-11-18T21:06:00.123456Z")
            .unwrap()
            .with_timezone(&Utc);
        let s = to_db(t);
        assert_eq!(s, "2026-11-18T21:06:00.123Z");
        let back = from_db("t", "c", &s).unwrap();
        assert_eq!(to_db(back), s);
        assert!(matches!(
            from_db("t", "c", "garbage"),
            Err(Error::Corrupt { .. })
        ));
    }
}
