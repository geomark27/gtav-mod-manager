//! Games the manager supports. Each [`Edition`] belongs to exactly one game.

use crate::Edition;

persisted_enum!(
    /// A supported game.
    Game, "game" {
        Gtav => "gtav",
        Rdr2 => "rdr2",
    }
);

impl Game {
    /// Human readable name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Gtav => "Grand Theft Auto V",
            Self::Rdr2 => "Red Dead Redemption 2",
        }
    }

    /// Editions of this game, in declaration order.
    pub fn editions(self) -> impl Iterator<Item = Edition> {
        Edition::ALL.into_iter().filter(move |e| e.game() == self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract() {
        crate::macros::assert_enum_contract(&Game::ALL, &["gtav", "rdr2"], Game::as_str);
    }

    #[test]
    fn every_edition_belongs_to_one_game() {
        let gtav: Vec<Edition> = Game::Gtav.editions().collect();
        let rdr2: Vec<Edition> = Game::Rdr2.editions().collect();
        assert_eq!(gtav, [Edition::Legacy, Edition::Enhanced]);
        assert_eq!(rdr2, [Edition::Rdr2]);
        assert_eq!(gtav.len() + rdr2.len(), Edition::ALL.len());
    }

    #[test]
    fn names() {
        assert_eq!(Game::Gtav.display_name(), "Grand Theft Auto V");
        assert_eq!(Game::Rdr2.display_name(), "Red Dead Redemption 2");
    }
}
