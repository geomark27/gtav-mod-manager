//! Private macros shared by the domain modules.

/// Declares a persisted enum with `as_str`, `FromStr`, `Display`, `ALL` and serde `snake_case`.
macro_rules! persisted_enum {
    (
        $(#[$meta:meta])*
        $name:ident, $kind:literal {
            $( $variant:ident => $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
        pub enum $name {
            $(
                #[doc = concat!("Persisted as `", $text, "`.")]
                $variant,
            )+
        }

        impl $name {
            /// Every value, in declaration order.
            pub const ALL: [Self; [$(Self::$variant),+].len()] = [$(Self::$variant),+];

            /// The persisted string (matches the SQL `CHECK` value).
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )+
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::Error;

            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                match s {
                    $( $text => Ok(Self::$variant), )+
                    other => Err($crate::Error::InvalidEnumValue {
                        kind: $kind,
                        value: other.to_owned(),
                    }),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

/// Test helper: checks `as_str`/`FromStr`/serde against the expected SQL `CHECK` list.
#[cfg(test)]
pub(crate) fn assert_enum_contract<T>(all: &[T], expected: &[&str], as_str: fn(T) -> &'static str)
where
    T: Copy
        + PartialEq
        + std::fmt::Debug
        + std::str::FromStr<Err = crate::Error>
        + serde::Serialize
        + serde::de::DeserializeOwned,
{
    let actual: Vec<&str> = all.iter().map(|v| as_str(*v)).collect();
    assert_eq!(actual, expected);
    for v in all {
        let s = as_str(*v);
        assert_eq!(s.parse::<T>(), Ok(*v));
        assert_eq!(serde_json::to_string(v).unwrap(), format!("\"{s}\""));
        let back: T = serde_json::from_str(&format!("\"{s}\"")).unwrap();
        assert_eq!(back, *v);
    }
    assert!(matches!(
        "bogus".parse::<T>(),
        Err(crate::Error::InvalidEnumValue { .. })
    ));
}
