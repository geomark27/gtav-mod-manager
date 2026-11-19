import { useId, useState } from "react";
import { Badge, type BadgeTone } from "../../components/Badge";
import { Button } from "../../components/Button";
import type { BaselineTrust } from "../../bindings/BaselineTrust";
import { formatBytes, formatDateTime } from "../../lib/format";
import { useProgress } from "../../lib/progress";
import { ErrorMessage } from "../installations/ErrorMessage";
import { useCancelOperation, useInventorySummary } from "../inventory/useInventory";
import { useCaptureBaseline } from "./useBaseline";
import styles from "./Baseline.module.css";

const TRUST_INFO: Record<BaselineTrust, { label: string; tone: BadgeTone }> = {
  none: { label: "No baseline", tone: "neutral" },
  untrusted: { label: "Untrusted", tone: "warning" },
  trusted: { label: "Trusted", tone: "success" },
};

/** Shows the baseline and lets the user capture a new one. */
export function BaselinePanel({ installationId }: { installationId: string }) {
  const summary = useInventorySummary(installationId);
  const capture = useCaptureBaseline(installationId);
  const cancel = useCancelOperation();
  const progress = useProgress("baseline");
  const [open, setOpen] = useState(false);
  const [verified, setVerified] = useState(false);
  const checkId = useId();

  const baseline = summary.data?.baseline;
  const running = capture.isPending;
  const cancelled = capture.error?.code === "CANCELLED";
  const trust = baseline ? TRUST_INFO[baseline.trust] : null;

  return (
    <div className={styles.panel}>
      <div className={styles.row}>
        <span>Baseline:</span>
        {trust ? <Badge tone={trust.tone}>{trust.label}</Badge> : null}
        {baseline && baseline.trust !== "none" ? (
          <span className={styles.muted}>
            {baseline.entries} files
            {baseline.capturedAt ? ` · captured ${formatDateTime(baseline.capturedAt)}` : ""}
          </span>
        ) : null}
        {!open ? (
          <Button onClick={() => setOpen(true)} disabled={running}>
            Capture baseline
          </Button>
        ) : null}
      </div>

      {open ? (
        <div className={styles.expanded}>
          <p>
            Reads and hashes every game file. This can take several minutes for a full
            installation.
          </p>
          <label htmlFor={checkId} className={styles.check}>
            <input
              id={checkId}
              type="checkbox"
              checked={verified}
              disabled={running}
              onChange={(e) => setVerified(e.target.checked)}
            />
            I verified the game files with the launcher (Steam / Epic / Rockstar) just now
          </label>
          <div className={styles.row}>
            <Button
              variant="primary"
              loading={running}
              onClick={() => capture.mutate(verified, { onSuccess: () => setOpen(false) })}
            >
              Start
            </Button>
            {running ? (
              <Button onClick={() => cancel.mutate()} disabled={cancel.isPending}>
                Cancel
              </Button>
            ) : (
              <Button variant="ghost" onClick={() => setOpen(false)}>
                Close
              </Button>
            )}
          </div>
          {running ? (
            <div role="status" aria-label="Baseline progress" className={styles.progress}>
              {progress && !progress.done
                ? `${progress.filesSeen} files read · ${formatBytes(progress.bytesHashed)} hashed`
                : "Starting…"}
              {progress?.current && !progress.done ? (
                <span className={styles.current}>{progress.current}</span>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}

      {capture.isSuccess && !open ? (
        <p className={styles.muted}>
          Baseline captured ({capture.data.entries} files). Scan again to apply it.
        </p>
      ) : null}
      {cancelled ? <p className={styles.muted}>Capture cancelled. The previous baseline was kept.</p> : null}
      {capture.error && !cancelled ? <ErrorMessage error={capture.error} /> : null}
      {cancel.error ? <ErrorMessage error={cancel.error} /> : null}
    </div>
  );
}
