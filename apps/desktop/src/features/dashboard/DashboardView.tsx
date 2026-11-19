import { Link } from "react-router";
import type { BaselineTrust } from "../../bindings/BaselineTrust";
import type { DashboardDto } from "../../bindings/DashboardDto";
import type { ScanStatusDto } from "../../bindings/ScanStatusDto";
import { Badge, type BadgeTone } from "../../components/Badge";
import { Card } from "../../components/Card";
import { formatBytes, formatDateTime } from "../../lib/format";
import { GuardBanner } from "../guards/GuardBanner";
import { guardNotice } from "../guards/blocking";
import { ErrorMessage } from "../installations/ErrorMessage";
import { STATUS_INFO } from "../activity/labels";
import { formatRelative } from "./relativeTime";
import styles from "./Dashboard.module.css";
import { useDashboard } from "./useDashboard";

const TRUST_INFO: Record<BaselineTrust, { label: string; tone: BadgeTone }> = {
  none: { label: "No baseline", tone: "neutral" },
  untrusted: { label: "Untrusted", tone: "warning" },
  trusted: { label: "Trusted", tone: "success" },
};

const SCAN_INFO: Record<ScanStatusDto, { label: string; tone: BadgeTone }> = {
  running: { label: "Running", tone: "info" },
  completed: { label: "Completed", tone: "success" },
  failed: { label: "Failed", tone: "danger" },
  cancelled: { label: "Cancelled", tone: "warning" },
};

function countOf(data: DashboardDto, classification: string): number {
  return data.inventory.find((c) => c.classification === classification)?.count ?? 0;
}

/** Statistics of one installation. */
export function DashboardView({ installationId }: { installationId: string }) {
  const dashboard = useDashboard(installationId);

  if (dashboard.isPending) return <p className={styles.muted}>Loading…</p>;
  if (dashboard.error) return <ErrorMessage error={dashboard.error} />;
  const data = dashboard.data;
  const notice = guardNotice(data.guard);
  const trust = TRUST_INFO[data.baseline.trust];
  const scan = data.lastScan;

  return (
    <div className={styles.stack}>
      {notice ? <GuardBanner notice={notice} /> : null}
      <div className={styles.grid}>
        <Card title="Mods">
          <div className={styles.value}>
            {data.mods.enabled} / {data.mods.total}
          </div>
          <div className={styles.detail}>enabled</div>
          {data.mods.drifted > 0 || data.mods.error > 0 ? (
            <div className={styles.badges}>
              {data.mods.drifted > 0 ? (
                <Badge tone="danger">{data.mods.drifted} drifted</Badge>
              ) : null}
              {data.mods.error > 0 ? <Badge tone="danger">{data.mods.error} in error</Badge> : null}
            </div>
          ) : null}
        </Card>
        <Card title="Inventory">
          <dl className={styles.detail}>
            <dt>Candidates</dt>
            <dd className={styles.value}>{countOf(data, "candidate")}</dd>
            <dt>Unknown</dt>
            <dd className={styles.value}>{countOf(data, "unknown")}</dd>
            <dt>Modified originals</dt>
            <dd className={styles.value}>{countOf(data, "original_modified")}</dd>
          </dl>
        </Card>
        <Card title="Last scan">
          {scan ? (
            <>
              <div className={styles.value}>{formatRelative(scan.startedAt)}</div>
              <div className={styles.badges}>
                <Badge tone={SCAN_INFO[scan.status].tone}>{SCAN_INFO[scan.status].label}</Badge>
              </div>
            </>
          ) : (
            <p className={styles.muted}>Never scanned.</p>
          )}
        </Card>
        <Card title="Baseline">
          <div className={styles.badges}>
            <Badge tone={trust.tone}>{trust.label}</Badge>
          </div>
          {data.baseline.trust !== "none" ? (
            <div className={styles.detail}>
              {data.baseline.entries} files
              {data.baseline.capturedAt
                ? ` · captured ${formatDateTime(data.baseline.capturedAt)}`
                : ""}
            </div>
          ) : null}
        </Card>
        <Card title="Vault usage">
          <div className={styles.value}>{formatBytes(data.vaultBytes)}</div>
        </Card>
      </div>

      <Card title="Recent activity">
        {data.recentTransactions.length === 0 ? (
          <p className={styles.muted}>No activity yet.</p>
        ) : (
          <ul className={styles.list}>
            {data.recentTransactions.map((t) => (
              <li key={t.id} className={styles.item}>
                <span>
                  {t.description} <span className={styles.muted}>· {formatRelative(t.createdAt)}</span>
                </span>
                <Badge tone={STATUS_INFO[t.status].tone}>{STATUS_INFO[t.status].label}</Badge>
              </li>
            ))}
          </ul>
        )}
        <p className={styles.detail}>
          <Link to="/activity">View all activity</Link>
        </p>
      </Card>
    </div>
  );
}
