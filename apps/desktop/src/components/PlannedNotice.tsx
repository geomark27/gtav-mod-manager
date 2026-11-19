import styles from "./PlannedNotice.module.css";

export interface PlannedNoticeProps {
  milestone: string;
  description: string;
}

/** Plain notice shown by screens whose milestone is not built yet. Contains no controls. */
export function PlannedNotice({ milestone, description }: PlannedNoticeProps) {
  return (
    <div className={styles.notice}>
      <p className={styles.title}>Planned for milestone {milestone}</p>
      <p className={styles.description}>{description}</p>
    </div>
  );
}
