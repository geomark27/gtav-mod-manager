import type { InstallationStatus } from "../../bindings/InstallationStatus";
import { Badge } from "../../components/Badge";
import { STATUS_INFO } from "./labels";

/** Badge for an installation status. */
export function StatusBadge({ status }: { status: InstallationStatus }) {
  const info = STATUS_INFO[status];
  return <Badge tone={info.tone}>{info.label}</Badge>;
}
