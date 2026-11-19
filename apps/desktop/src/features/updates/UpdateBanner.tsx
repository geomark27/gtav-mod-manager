import { Button } from "../../components/Button";
import { formatBytes } from "../../lib/format";
import styles from "./Updates.module.css";
import { useUpdater } from "./useUpdater";

/** Reason shown while a backend operation keeps the update from restarting the app. */
export const BUSY_REASON = "Wait for the current operation to finish.";

/** Offers the available update at the top of the main area. */
export function UpdateBanner() {
  const {
    phase,
    available,
    downloaded,
    total,
    error,
    dismissed,
    operationBusy,
    install,
    dismiss,
  } = useUpdater();

  if (!available || dismissed) return null;

  const installing = phase === "downloading" || phase === "ready";
  const percent = total && total > 0 ? Math.min(100, Math.floor((downloaded / total) * 100)) : null;

  return (
    <div className={styles.banner} role="status" aria-live="polite" aria-label="Application update">
      <div className={styles.head}>
        <p className={styles.title}>Version {available.version} is available.</p>
        <Button variant="ghost" onClick={dismiss} disabled={installing}>
          Later
        </Button>
      </div>

      {available.notes ? (
        <details className={styles.notes}>
          <summary>Release notes</summary>
          <p>{available.notes}</p>
        </details>
      ) : null}

      {phase === "downloading" ? (
        <div className={styles.progressBlock}>
          <div
            className={styles.progress}
            role="progressbar"
            aria-label="Update download"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent ?? undefined}
          >
            <div className={styles.progressBar} style={{ width: `${percent ?? 0}%` }} />
          </div>
          <p className={styles.progressText}>
            {total
              ? `${formatBytes(downloaded)} of ${formatBytes(total)}`
              : `${formatBytes(downloaded)} downloaded`}
          </p>
        </div>
      ) : null}

      {phase === "ready" ? <p className={styles.muted}>Installed. Restarting…</p> : null}
      {error ? (
        <p className={styles.error} role="alert">
          {error}
        </p>
      ) : null}

      <div className={styles.actions}>
        {operationBusy ? <p className={styles.blocked}>{BUSY_REASON}</p> : null}
        <Button
          variant="primary"
          loading={phase === "downloading"}
          disabled={operationBusy || installing}
          onClick={() => {
            void install();
          }}
        >
          Update now
        </Button>
      </div>
    </div>
  );
}
