import type { Game } from "../../bindings/Game";

/** Name of the app. The install identity (installer name, data folder) keeps its old name. */
export const APP_NAME = "ModSwitch";

/** Supported games, in display order. */
export const GAMES: readonly Game[] = ["gtav", "rdr2"];

/** Full name of each game. */
export const GAME_NAME: Record<Game, string> = {
  gtav: "Grand Theft Auto V",
  rdr2: "Red Dead Redemption 2",
};

/** Short name of each game, for sentences and badges. */
export const GAME_SHORT: Record<Game, string> = {
  gtav: "GTA V",
  rdr2: "Red Dead Redemption 2",
};

/** Name of each game's online mode. */
export const ONLINE_NAME: Record<Game, string> = {
  gtav: "GTA Online",
  rdr2: "Red Dead Online",
};

/** Persistent story-mode notice (architecture §12), shared by the sidebar and Settings. */
export const ONLINE_NOTICE =
  "Mods managed here are for story mode. Disabling mods does not make an installation safe for GTA Online or Red Dead Online.";

/** Groups items by game in [`GAMES`] order, leaving out games without items. */
export function groupByGame<T extends { game: Game }>(items: readonly T[]): { game: Game; items: T[] }[] {
  return GAMES.map((game) => ({ game, items: items.filter((i) => i.game === game) })).filter(
    (g) => g.items.length > 0,
  );
}
