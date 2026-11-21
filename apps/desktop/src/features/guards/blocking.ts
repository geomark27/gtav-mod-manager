import type { GuardStatusDto } from "../../bindings/GuardStatusDto";

export type GuardSeverity = "warning" | "danger";

export interface GuardNotice {
  severity: GuardSeverity;
  text: string;
}

/**
 * The reason mods cannot be changed right now, or `null` when writes are allowed.
 * The first failing guard wins, in the same order as the backend.
 */
export function guardNotice(guard: GuardStatusDto): GuardNotice | null {
  if (guard.gameRunning.length > 0) {
    return {
      severity: "warning",
      text: `The game or a launcher is running (${guard.gameRunning.join(", ")}). Close it to change mods.`,
    };
  }
  if (guard.installationMissing) {
    return {
      severity: "warning",
      text: "The installation folder was not found. Check that the drive is connected.",
    };
  }
  if (guard.recoveryRequired) {
    return {
      severity: "danger",
      text: "A previous change did not finish cleanly, so changes are blocked to protect your files. Restart the app so it can recover the installation.",
    };
  }
  if (!guard.writable) {
    return {
      severity: "warning",
      text: "The game folder is not writable. Run the app with permission to write to it.",
    };
  }
  return null;
}
