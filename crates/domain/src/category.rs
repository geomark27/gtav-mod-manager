//! Mod categories.

persisted_enum!(
    /// Category of a mod.
    Category, "category" {
        GraphicsTextures => "graphics_textures",
        Vehicles => "vehicles",
        ScriptsGameplay => "scripts_gameplay",
        MapsBuildings => "maps_buildings",
        CharactersClothing => "characters_clothing",
        SoundsMusic => "sounds_music",
        Uncategorized => "uncategorized",
    }
);

impl Category {
    /// Human readable name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::GraphicsTextures => "Graphics & Textures",
            Self::Vehicles => "Vehicles",
            Self::ScriptsGameplay => "Scripts & Gameplay",
            Self::MapsBuildings => "Maps & Buildings",
            Self::CharactersClothing => "Characters & Clothing",
            Self::SoundsMusic => "Sounds & Music",
            Self::Uncategorized => "Uncategorized",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract() {
        crate::macros::assert_enum_contract(
            &Category::ALL,
            &[
                "graphics_textures",
                "vehicles",
                "scripts_gameplay",
                "maps_buildings",
                "characters_clothing",
                "sounds_music",
                "uncategorized",
            ],
            Category::as_str,
        );
    }

    #[test]
    fn display_names() {
        let names: Vec<_> = Category::ALL.iter().map(|c| c.display_name()).collect();
        assert_eq!(
            names,
            [
                "Graphics & Textures",
                "Vehicles",
                "Scripts & Gameplay",
                "Maps & Buildings",
                "Characters & Clothing",
                "Sounds & Music",
                "Uncategorized"
            ]
        );
    }
}
