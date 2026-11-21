import { Link } from "react-router";
import type { OnlineCheckDto } from "../../bindings/OnlineCheckDto";
import type { ReadinessCheckDto } from "../../bindings/ReadinessCheckDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { formatDateTime } from "../../lib/format";
import { ONLINE_NOTICE } from "../../layout/AppShell";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Online.module.css";
import { STATUS_LABEL, STATUS_TONE, checkTitle, describeCheck, verdictText } from "./labels";
import { useCheckOnline, usePrepareOnline } from "./useOnline";

export interface OnlineCheckPanelProps {
  installationId: string;
  /** Why changes to the game folder are blocked right now, or `null`. */
  blockedReason: string | null;
}

/**
 * Before playing online: disable every mod and list what is left in the game folder, or only
 * check. It never states that playing online is safe.
 */
export function OnlineCheckPanel({ installationId, blockedReason }: OnlineCheckPanelProps) {
  const check = useCheckOnline(installationId);
  const prepare = usePrepareOnline(installationId);
  const busy = check.isPending || prepare.isPending;
  // The most recent answer wins.
  const result =
    (check.submittedAt > prepare.submittedAt ? check.data : prepare.data) ??
    check.data ??
    prepare.data;
  const error = check.submittedAt > prepare.submittedAt ? check.error : prepare.error;

  return (
    <Card title="Before playing online">
      <div className={styles.stack}>
        <p className={styles.muted}>
          Prepare for online disables every managed mod (saving a restore point), scans the game
          folder and lists anything left that could be a mod. Check only does the scan and the
          list without changing anything.
        </p>
        <div className={styles.actions}>
          <Button
            variant="primary"
            loading={prepare.isPending}
            disabled={busy || blockedReason !== null}
            title={blockedReason ?? undefined}
            onClick={() => prepare.mutate()}
          >
            Prepare for online
          </Button>
          <Button loading={check.isPending} disabled={busy} onClick={() => check.mutate()}>
            Check only
          </Button>
        </div>
        {error ? <ErrorMessage error={error} /> : null}
        {result ? <Result result={result} /> : null}
        <p className={styles.notice} role="note">
          {ONLINE_NOTICE}
        </p>
      </div>
    </Card>
  );
}

function Result({ result }: { result: OnlineCheckDto }) {
  const v = verdictText(result);
  return (
    <div className={styles.stack}>
      {result.vanillaApplied ? (
        <p className={styles.success} role="status">
          Every managed mod was disabled. Use Restore my mods to bring them back after playing.
        </p>
      ) : null}
      {result.actionError ? (
        <div>
          <p className={styles.muted}>The mods were not disabled:</p>
          <ErrorMessage error={result.actionError} />
        </div>
      ) : null}
      <div className={styles.verdict} data-tone={v.tone} aria-label="Online check result">
        <span className={styles.verdictTitle}>{v.title}</span>
        <span className={styles.muted}>
          {v.sub} Checked {formatDateTime(result.checkedAt)}.
        </span>
      </div>
      <ul className={styles.checks}>
        {result.checks.map((c) => (
          <CheckRow key={c.id} check={c} result={result} />
        ))}
      </ul>
    </div>
  );
}

function CheckRow({ check: c, result }: { check: ReadinessCheckDto; result: OnlineCheckDto }) {
  const d = describeCheck(c, result);
  const more = c.count - c.items.length;
  return (
    <li className={styles.check} aria-label={checkTitle(c.id, result.game)}>
      <div className={styles.checkHead}>
        <span>{checkTitle(c.id, result.game)}</span>
        <Badge tone={STATUS_TONE[c.status]}>{STATUS_LABEL[c.status]}</Badge>
      </div>
      <span className={styles.muted}>{d.detail}</span>
      {d.fix ? (
        <span className={styles.muted}>
          {d.fix} {d.link ? <Link to={d.link.to}>{d.link.label}</Link> : null}
        </span>
      ) : null}
      {c.status !== "pass" && c.items.length > 0 && c.id !== "game_closed" ? (
        <details className={styles.items}>
          <summary>
            {c.id === "managed_mods" ? "Mods" : "Files"} ({c.count})
          </summary>
          <ul>
            {c.items.map((i) => (
              <li key={i}>{i}</li>
            ))}
          </ul>
          {more > 0 ? <p className={styles.muted}>and {more} more</p> : null}
        </details>
      ) : null}
    </li>
  );
}
