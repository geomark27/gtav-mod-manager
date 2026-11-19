import type { BadgeTone } from "../../components/Badge";
import type { ModSwitchState } from "../../components/ModSwitch";
import type { ModState } from "../../bindings/ModState";

export const MOD_STATE_INFO: Record<ModState, { label: string; tone: BadgeTone }> = {
  enabled: { label: "Enabled", tone: "success" },
  disabled: { label: "Disabled", tone: "neutral" },
  drifted: { label: "Drifted", tone: "danger" },
  error: { label: "Error", tone: "danger" },
};

/** Switch state derived only from the server state (D-11); `pending` and `blocked` override. */
export function switchState(
  state: ModState,
  pending: boolean,
  blockedReason: string | null,
): ModSwitchState {
  if (pending) return "pending";
  if (blockedReason !== null) return "blocked";
  switch (state) {
    case "enabled":
      return "on";
    case "disabled":
      return "off";
    case "drifted":
    case "error":
      return "error";
  }
}

/** Explanation shown under a mod that is not cleanly enabled or disabled. */
export function stateProblem(state: ModState): string | null {
  if (state === "drifted") return "Files changed outside the app";
  if (state === "error") return "The stored copy of this mod's files is damaged.";
  return null;
}
