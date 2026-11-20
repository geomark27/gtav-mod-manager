/** Reminder that vanilla mode only removes managed mods; `null` when nothing is left over. */
export function unmanagedNote(unmanagedFiles: number | null): string | null {
  if (unmanagedFiles === null || unmanagedFiles === 0) return null;
  const files = unmanagedFiles === 1 ? "1 file" : `${unmanagedFiles} files`;
  return `${files} the app does not manage stay in the game folder (candidates, unknown files or modified originals). Adopt them as mods so vanilla mode can remove them too.`;
}
