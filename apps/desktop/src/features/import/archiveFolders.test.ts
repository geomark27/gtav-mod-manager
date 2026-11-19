import { archiveFolders } from "./archiveFolders";

describe("archiveFolders", () => {
  it("lists every folder prefix once, sorted", () => {
    expect(archiveFolders(["Mod/scripts/a.asi", "Mod/b.dll", "readme.txt", "Mod/scripts/x/y.ini"])).toEqual([
      "Mod",
      "Mod/scripts",
      "Mod/scripts/x",
    ]);
  });

  it("returns nothing for a flat archive", () => {
    expect(archiveFolders(["a.asi", "b.ini"])).toEqual([]);
  });
});
