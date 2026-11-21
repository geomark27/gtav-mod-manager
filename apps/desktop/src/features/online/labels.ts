import type { BadgeTone } from "../../components/Badge";
import type { CheckStatus } from "../../bindings/CheckStatus";
import type { Game } from "../../bindings/Game";
import type { OnlineCheckDto } from "../../bindings/OnlineCheckDto";
import type { ReadinessCheckDto } from "../../bindings/ReadinessCheckDto";
import type { ReadinessCheckId } from "../../bindings/ReadinessCheckId";

export const STATUS_TONE: Record<CheckStatus, BadgeTone> = {
  pass: "success",
  warn: "warning",
  fail: "danger",
};

export const STATUS_LABEL: Record<CheckStatus, string> = {
  pass: "OK",
  warn: "Review",
  fail: "Fix",
};

/** Short name of each game. */
export const GAME_SHORT: Record<Game, string> = {
  gtav: "GTA V",
  rdr2: "Red Dead Redemption 2",
};

/** Name of each game's online mode. */
export const ONLINE_NAME: Record<Game, string> = {
  gtav: "GTA Online",
  rdr2: "Red Dead Online",
};

/** Name of each check, as a statement of what is checked. */
export function checkTitle(id: ReadinessCheckId, game: Game): string {
  return id === "game_closed" ? `${GAME_SHORT[game]} and its launchers are closed` : CHECK_TITLE[id];
}

const CHECK_TITLE: Record<Exclude<ReadinessCheckId, "game_closed">, string> = {
  recovery: "No interrupted change is waiting",
  managed_mods: "No managed mod is in the game folder",
  mod_files: "No other mod files in the game folder",
  unknown_files: "No unrecognized files",
  originals: "Original game files are unchanged",
  leftovers: "No files left by an interrupted change",
};

/** Where to look when a link helps fix a check. */
export type FixLink = { to: string; label: string } | null;

/** What a check found, in one sentence, and how to fix it. `null` fix when it passed. */
export function describeCheck(
  c: ReadinessCheckDto,
  result: Pick<OnlineCheckDto, "baseline">,
): { detail: string; fix: string | null; link: FixLink } {
  const n = c.count;
  const files = n === 1 ? "1 file" : `${n} files`;
  if (c.status === "pass") {
    if (c.id === "originals") {
      return {
        detail:
          result.baseline === "trusted"
            ? "Every original matches the trusted baseline."
            : "Every original matches the baseline. It is not marked as trusted, so it only shows changes since it was captured.",
        fix: null,
        link: null,
      };
    }
    return { detail: "Nothing found.", fix: null, link: null };
  }
  switch (c.id) {
    case "game_closed":
      return { detail: `Running: ${c.items.join(", ")}.`, fix: "Close them, then check again.", link: null };
    case "recovery":
      return {
        detail: "A change to the game folder did not finish cleanly.",
        fix: "Restart the app so it can finish or undo that change.",
        link: null,
      };
    case "managed_mods":
      return {
        detail: `${n === 1 ? "1 mod has" : `${n} mods have`} files in the game folder.`,
        fix: c.items.some((i) => i.endsWith(")"))
          ? "Resolve the drifted or damaged mods on the Mods page, then prepare again."
          : "Prepare for online disables them in one step and saves a restore point.",
        link: c.items.some((i) => i.endsWith(")")) ? { to: "/mods", label: "Open Mods" } : null,
      };
    case "mod_files":
      return {
        detail: `${files} that look like mods ${n === 1 ? "is" : "are"} not managed by the app, so vanilla mode cannot remove ${n === 1 ? "it" : "them"}.`,
        fix: "Adopt them in Review → Detected mods, then prepare again. Files marked “in the baseline” were captured into a trusted baseline; capture it again on a clean game.",
        link: { to: "/review", label: "Open Review" },
      };
    case "unknown_files":
      return {
        detail: `${files} the app does not recognize. They may be harmless (notes, logs) or parts of a mod.`,
        fix: "Look at them in Review → All files and adopt the ones that belong to a mod.",
        link: { to: "/review", label: "Open Review" },
      };
    case "originals":
      if (n === 0) {
        return {
          detail: "There is no baseline, so the app cannot tell whether an original file changed.",
          fix: "Verify the game files in your launcher, then capture a trusted baseline in Review → Baseline.",
          link: { to: "/review", label: "Open Review" },
        };
      }
      return {
        detail: `${files} of the game differ from the baseline. The app never repairs original files.`,
        fix: "Verify the game files in Steam, Epic Games or the Rockstar Games Launcher, then check again.",
        link: null,
      };
    case "leftovers":
      return {
        detail: `${files} left by an interrupted change. Their names end in .gmm-tmp or .gmm-tomb, so the game does not load them.`,
        fix: "Review them in Review → All files before deleting anything.",
        link: { to: "/review", label: "Open Review" },
      };
  }
}

/** The headline of a result. */
export function verdictText(result: OnlineCheckDto): { title: string; sub: string; tone: BadgeTone } {
  const fails = result.checks.filter((c) => c.status === "fail").length;
  const warns = result.checks.filter((c) => c.status === "warn").length;
  switch (result.verdict) {
    case "clear":
      return {
        title: "No mod files found",
        sub: `Based on these checks only. This is not a guarantee for ${ONLINE_NAME[result.game]}.`,
        tone: "success",
      };
    case "review":
      return {
        title: warns === 1 ? "1 thing to review" : `${warns} things to review`,
        sub: "Nothing must be fixed, but look at these before you play online.",
        tone: "warning",
      };
    case "blocked":
      return {
        title: fails === 1 ? "1 thing to fix" : `${fails} things to fix`,
        sub: `Fix these before you open ${ONLINE_NAME[result.game]}.`,
        tone: "danger",
      };
  }
}
