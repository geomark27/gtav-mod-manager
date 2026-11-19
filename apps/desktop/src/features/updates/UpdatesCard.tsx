import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { formatDateTime } from "../../lib/format";
import settings from "../settings/Settings.module.css";
import { useUpdater } from "./useUpdater";

/** Settings card: installed version, last check and a manual update check. */
export function UpdatesCard() {
  const { phase, available, error, lastCheckAt, checkNow } = useUpdater();
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getVersion()
      .then((v) => {
        if (active) setVersion(v);
      })
      .catch(() => {
        // The About card already shows the version reported by the backend.
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <Card title="Updates">
      <div className={settings.stack}>
        <dl className={settings.rows}>
          <dt className={settings.label}>Installed version</dt>
          <dd className={settings.value}>{version ?? "…"}</dd>
          <dt className={settings.label}>Last check</dt>
          <dd className={settings.value}>
            {lastCheckAt ? formatDateTime(lastCheckAt) : "Not checked yet"}
          </dd>
        </dl>

        <div>
          <Button
            loading={phase === "checking"}
            onClick={() => {
              void checkNow();
            }}
          >
            Check for updates
          </Button>
        </div>

        {phase === "checking" ? <p className={settings.muted}>Checking for updates…</p> : null}
        {error ? (
          <p className={settings.error} role="alert">
            {error}
          </p>
        ) : null}
        {!error && available ? (
          <p role="status">
            Version {available.version} is available. Use the banner at the top of the window to
            install it.
          </p>
        ) : null}
        {!error && !available && lastCheckAt && phase !== "checking" ? (
          <p role="status">You have the latest version.</p>
        ) : null}
      </div>
    </Card>
  );
}
