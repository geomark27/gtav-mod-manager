import type { AppError } from "../../bindings/AppError";
import { Badge } from "../../components/Badge";
import styles from "./Installations.module.css";

/** Inline error with code, message and hint. */
export function ErrorMessage({ error }: { error: AppError }) {
  return (
    <div className={styles.error} role="alert">
      <Badge tone="danger">{error.code}</Badge>
      <p>{error.message}</p>
      {error.hint ? <p className={styles.muted}>{error.hint}</p> : null}
      {error.details ? <p className={styles.muted}>{error.details}</p> : null}
    </div>
  );
}
