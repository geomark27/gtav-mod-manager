import { describe, expect, it } from "vitest";
import { ONLINE_NOTICE, groupByGame } from "./games";

describe("games", () => {
  it("groups by game in display order and drops empty games", () => {
    const items = [
      { id: "a", game: "rdr2" as const },
      { id: "b", game: "gtav" as const },
      { id: "c", game: "rdr2" as const },
    ];
    expect(groupByGame(items)).toEqual([
      { game: "gtav", items: [{ id: "b", game: "gtav" }] },
      {
        game: "rdr2",
        items: [
          { id: "a", game: "rdr2" },
          { id: "c", game: "rdr2" },
        ],
      },
    ]);
    expect(groupByGame(items.filter((i) => i.game === "rdr2")).map((g) => g.game)).toEqual(["rdr2"]);
    expect(groupByGame([])).toEqual([]);
  });

  it("never says that online play is safe", () => {
    expect(ONLINE_NOTICE).toMatch(/does not make an installation safe/);
    expect(ONLINE_NOTICE).toMatch(/GTA Online/);
    expect(ONLINE_NOTICE).toMatch(/Red Dead Online/);
  });
});
