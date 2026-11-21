//! Game editions. GTA V has two (Legacy and Enhanced); Red Dead Redemption 2 has one.

use crate::Game;

persisted_enum!(
    /// An edition of a supported game.
    Edition, "edition" {
        Legacy => "legacy",
        Enhanced => "enhanced",
        Rdr2 => "rdr2",
    }
);

impl Edition {
    /// Game this edition belongs to.
    pub const fn game(self) -> Game {
        match self {
            Self::Legacy | Self::Enhanced => Game::Gtav,
            Self::Rdr2 => Game::Rdr2,
        }
    }

    /// Human readable name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Legacy => "GTA V Legacy",
            Self::Enhanced => "GTA V Enhanced",
            Self::Rdr2 => "Red Dead Redemption 2",
        }
    }

    /// Name of the game executable that identifies the edition.
    ///
    /// ASSUMPTIONS A-01 and A-09, see docs/roadmap.md.
    pub const fn executable_name(self) -> &'static str {
        match self {
            Self::Legacy => "GTA5.exe",
            Self::Enhanced => "GTA5_Enhanced.exe",
            Self::Rdr2 => "RDR2.exe",
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
            &["legacy", "enhanced", "rdr2"],
            Edition::as_str,
        );
    }

    #[test]
    fn names() {
        assert_eq!(Edition::Legacy.display_name(), "GTA V Legacy");
        assert_eq!(Edition::Enhanced.display_name(), "GTA V Enhanced");
        assert_eq!(Edition::Rdr2.display_name(), "Red Dead Redemption 2");
        assert_eq!(Edition::Legacy.executable_name(), "GTA5.exe");
        assert_eq!(Edition::Enhanced.executable_name(), "GTA5_Enhanced.exe");
        assert_eq!(Edition::Rdr2.executable_name(), "RDR2.exe");
        assert_eq!(Edition::ALL.len(), 3);
    }

    #[test]
    fn games() {
        assert_eq!(Edition::Legacy.game(), Game::Gtav);
        assert_eq!(Edition::Enhanced.game(), Game::Gtav);
        assert_eq!(Edition::Rdr2.game(), Game::Rdr2);
    }
}
