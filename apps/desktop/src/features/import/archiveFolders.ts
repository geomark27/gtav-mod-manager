/** Every folder of an archive (all path prefixes), sorted, without a trailing slash. */
export function archiveFolders(paths: string[]): string[] {
  const folders = new Set<string>();
  for (const path of paths) {
    const parts = path.split("/").filter((p) => p !== "");
    for (let i = 1; i < parts.length; i += 1) folders.add(parts.slice(0, i).join("/"));
  }
  return [...folders].sort((a, b) => a.localeCompare(b));
}
