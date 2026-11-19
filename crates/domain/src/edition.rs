//! GTA V edition.

persisted_enum!(
    /// A GTA V edition.
    Edition, "edition" {
        Legacy => "legacy",
        Enhanced => "enhanced",
    }
);

impl Edition {
    /// Human readable name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Legacy => "GTA V Legacy",
            Self::Enhanced => "GTA V Enhanced",
        }
    }

    /// Name of the game executable that identifies the edition.
    ///
    /// ASSUMPTION A-01, see docs/roadmap.md.
    pub const fn executable_name(self) -> &'static str {
        match self {
            Self::Legacy => "GTA5.exe",
            Self::Enhanced => "GTA5_Enhanced.exe",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract() {
        crate::macros::assert_enum_contract(
            &Edition::ALL,
            &["legacy", "enhanced"],
            Edition::as_str,
        );
    }

    #[test]
    fn names() {
        assert_eq!(Edition::Legacy.display_name(), "GTA V Legacy");
        assert_eq!(Edition::Enhanced.display_name(), "GTA V Enhanced");
        assert_eq!(Edition::Legacy.executable_name(), "GTA5.exe");
        assert_eq!(Edition::Enhanced.executable_name(), "GTA5_Enhanced.exe");
        assert_eq!(Edition::ALL.len(), 2);
    }
}
