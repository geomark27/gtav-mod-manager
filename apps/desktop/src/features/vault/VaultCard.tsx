import { useState } from "react";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { Modal } from "../../components/Modal";
import { formatBytes } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "../settings/Settings.module.css";
import { useVaultGc, useVaultReport } from "./useVault";

/** Vault maintenance: size, unreferenced copies (deletable) and stray files (read-only). */
export function VaultCard() {
  const report = useVaultReport();
  const gc = useVaultGc();
  const [confirming, setConfirming] = useState(false);
  const [freed, setFreed] = useState<number | null>(null);
  const data = report.data;

  return (
    <Card title="Vault maintenance">
      {report.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {report.error ? <ErrorMessage error={report.error} /> : null}
      {data ? (
        <div className={styles.stack}>
          <dl className={styles.rows}>
            <dt className={styles.label}>Stored objects</dt>
            <dd className={styles.value}>
              {data.objectCount} ({formatBytes(data.totalBytes)})
            </dd>
            <dt className={styles.label}>Unreferenced copies</dt>
            <dd className={styles.value}>
              {data.unreferenced.length} ({formatBytes(data.unreferencedBytes)})
            </dd>
          </dl>
          <p className={styles.muted}>
            Includes copies kept when you restored a mod or re-adopted files.
          </p>
          {freed !== null ? <p role="status">Freed {formatBytes(freed)}.</p> : null}
          {data.unreferenced.length > 0 ? (
            <div>
              <Button
                variant="danger"
                onClick={() => {
                  gc.reset();
                  setFreed(null);
                  setConfirming(true);
                }}
              >
                Delete unreferenced copies…
              </Button>
            </div>
          ) : null}
          {data.strayFiles.length > 0 ? (
            <div className={styles.stack}>
              <p className={styles.muted}>
                These files in the vault folder are not known to the app. They are never deleted
                automatically.
              </p>
              <ul className={styles.mono} aria-label="Stray files">
                {data.strayFiles.map((f) => (
                  <li key={f}>{f}</li>
                ))}
              </ul>
            </div>
          ) : null}
          <Modal
            open={confirming}
            onOpenChange={(next) => {
              if (!next) setConfirming(false);
            }}
            dismissible={!gc.isPending}
            title="Delete unreferenced copies"
            description="These vault copies are not used by any mod."
          >
            <div className={styles.stack}>
              <p>
                Delete {data.unreferenced.length}{" "}
                {data.unreferenced.length === 1 ? "copy" : "copies"} (
                {formatBytes(data.unreferencedBytes)})? This cannot be undone. Files in the game
                folder are not touched.
              </p>
              {gc.error ? <ErrorMessage error={gc.error} /> : null}
              <div className={styles.actions}>
                <Button onClick={() => setConfirming(false)} disabled={gc.isPending}>
                  Cancel
                </Button>
                <Button
                  variant="danger"
                  loading={gc.isPending}
                  onClick={() =>
                    gc.mutate(
                      data.unreferenced.map((o) => o.sha256),
                      {
                        onSuccess: (bytes) => {
                          setFreed(bytes);
                          setConfirming(false);
                        },
                      },
                    )
                  }
                >
                  Delete copies
                </Button>
              </div>
            </div>
          </Modal>
        </div>
      ) : null}
    </Card>
  );
}
