import type { BadgeTone } from "../../components/Badge";
import type { DistributionPlatform } from "../../bindings/DistributionPlatform";
import type { Edition } from "../../bindings/Edition";
import type { InstallationStatus } from "../../bindings/InstallationStatus";

export const STATUS_INFO: Record<InstallationStatus, { label: string; tone: BadgeTone }> = {
  ok: { label: "Ready", tone: "success" },
  incomplete: { label: "Incomplete", tone: "warning" },
  missing: { label: "Not found", tone: "danger" },
  not_writable: { label: "Read-only", tone: "warning" },
};

export const EDITION_LABEL: Record<Edition, string> = {
  legacy: "Legacy",
  enhanced: "Enhanced",
};

export const PLATFORM_LABEL: Record<DistributionPlatform, string> = {
  steam: "Steam",
  epic: "Epic",
  rockstar: "Rockstar",
  unknown: "Unknown platform",
};

export const READ_ONLY_NOTICE =
  "This folder is protected. The app can inspect it but cannot change files without administrator rights, which this version does not use.";
