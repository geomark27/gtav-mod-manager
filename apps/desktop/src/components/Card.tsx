import type { ReactNode } from "react";
import styles from "./Card.module.css";

export interface CardProps {
  title?: string;
  children: ReactNode;
}

/** Surface container with an optional title. */
export function Card({ title, children }: CardProps) {
  return (
    <section className={styles.card} aria-label={title}>
      {title ? <h2 className={styles.title}>{title}</h2> : null}
      {children}
    </section>
  );
}
