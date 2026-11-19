import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { formatBytes, formatDateTime } from "../../lib/format";
import { useProgress } from "../../lib/progress";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Inventory.module.css";
import { useCancelOperation, useInventorySummary, useScan } from "./useInventory";

const STATUS_TEXT = {
  running: "Running",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
} as const;

/** Starts and cancels a scan, shows live progress and the last scan. */
export function ScanControl({ installationId }: { installationId: string }) {
  const scan = useScan(installationId);
  const cancel = useCancelOperation();
  const summary = useInventorySummary(installationId);
  const progress = useProgress("scan");
  const running = scan.isPending;
  const lastScan = summary.data?.lastScan ?? null;
  const cancelled = scan.error?.code === "CANCELLED";

  return (
    <div className={styles.stack}>
      <div className={styles.row}>
        <Button variant="primary" loading={running} onClick={() => scan.mutate()}>
          Scan now
        </Button>
        {running ? (
          <Button variant="secondary" onClick={() => cancel.mutate()} disabled={cancel.isPending}>
            Cancel
          </Button>
        ) : null}
        {lastScan ? (
          <span className={styles.muted}>
            Last scan: {formatDateTime(lastScan.startedAt)}{" "}
            <Badge tone={lastScan.status === "completed" ? "success" : "neutral"}>
              {STATUS_TEXT[lastScan.status]}
            </Badge>
          </span>
        ) : (
          <span className={styles.muted}>Not scanned yet.</span>
        )}
      </div>
      {running ? (
        <div className={styles.stack} role="status" aria-label="Scan progress">
          <div className={styles.bar} role="progressbar" aria-label="Scanning" />
          <span>
            {progress && !progress.done
              ? `${progress.filesSeen} files seen · ${formatBytes(progress.bytesHashed)} hashed`
              : "Starting scan…"}
          </span>
          {progress?.current && !progress.done ? (
            <span className={styles.current}>{progress.current}</span>
          ) : null}
        </div>
      ) : null}
      {cancelled ? <p className={styles.muted}>Scan cancelled. The previous results were kept.</p> : null}
      {scan.error && !cancelled ? <ErrorMessage error={scan.error} /> : null}
      {cancel.error ? <ErrorMessage error={cancel.error} /> : null}
    </div>
  );
}
