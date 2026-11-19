import { Fragment, useState } from "react";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { formatDateTime } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Activity.module.css";
import { KIND_LABEL, STATUS_INFO } from "./labels";
import { PAGE_SIZE, useTransactions } from "./useActivity";

/** Paginated history of the transactions of one installation. */
export function ActivityView({ installationId }: { installationId: string }) {
  const [page, setPage] = useState(0);
  const [expanded, setExpanded] = useState<string | null>(null);
  const history = useTransactions(installationId, page);

  if (history.isPending) return <p className={styles.muted}>Loading…</p>;
  if (history.error && !history.data) return <ErrorMessage error={history.error} />;
  const data = history.data;
  if (!data) return null;
  if (data.total === 0) return <p className={styles.muted}>No activity yet.</p>;

  const pages = Math.max(1, Math.ceil(data.total / PAGE_SIZE));
  return (
    <div className={styles.stack}>
      {history.error ? <ErrorMessage error={history.error} /> : null}
      <table className={styles.table}>
        <thead>
          <tr>
            <th>Time</th>
            <th>Description</th>
            <th>Kind</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {data.items.map((t) => {
            const open = expanded === t.id;
            const status = STATUS_INFO[t.status];
            return (
              <Fragment key={t.id}>
                <tr>
                  <td>{formatDateTime(t.createdAt)}</td>
                  <td>
                    <button
                      type="button"
                      className={styles.toggle}
                      aria-expanded={open}
                      onClick={() => setExpanded(open ? null : t.id)}
                    >
                      {t.description}
                    </button>
                    {t.error ? <div className={styles.errorText}>{t.error}</div> : null}
                  </td>
                  <td>{KIND_LABEL[t.kind]}</td>
                  <td>
                    <Badge tone={status.tone}>{status.label}</Badge>
                  </td>
                </tr>
                {open ? (
                  <tr>
                    <td colSpan={4}>
                      <div className={styles.detail}>
                        <span>
                          Mods: {t.modNames.length > 0 ? t.modNames.join(", ") : "none"}
                        </span>
                        <span>
                          {t.stepCount} file {t.stepCount === 1 ? "step" : "steps"}
                        </span>
                      </div>
                    </td>
                  </tr>
                ) : null}
              </Fragment>
            );
          })}
        </tbody>
      </table>
      <div className={styles.pager}>
        <Button
          onClick={() => {
            setExpanded(null);
            setPage(page - 1);
          }}
          disabled={page === 0}
        >
          Previous
        </Button>
        <span>
          Page {page + 1} of {pages}
        </span>
        <Button
          onClick={() => {
            setExpanded(null);
            setPage(page + 1);
          }}
          disabled={page + 1 >= pages}
        >
          Next
        </Button>
        <span className={styles.muted}>{data.total} total</span>
      </div>
    </div>
  );
}
