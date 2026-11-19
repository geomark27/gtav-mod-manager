import type { TransactionKind } from "../../bindings/TransactionKind";
import type { TransactionStatus } from "../../bindings/TransactionStatus";
import type { BadgeTone } from "../../components/Badge";

export const STATUS_INFO: Record<TransactionStatus, { label: string; tone: BadgeTone }> = {
  committed: { label: "Done", tone: "success" },
  rolled_back: { label: "Rolled back", tone: "warning" },
  needs_attention: { label: "Needs attention", tone: "danger" },
  open: { label: "In progress", tone: "info" },
  committing: { label: "In progress", tone: "info" },
  rolling_back: { label: "Rolling back", tone: "info" },
};

export const KIND_LABEL: Record<TransactionKind, string> = {
  adopt: "Adopt",
  enable: "Enable",
  disable: "Disable",
  group: "Category",
  profile: "Profile",
  restore: "Restore",
};
