import * as Dialog from "@radix-ui/react-dialog";
import type { ReactNode } from "react";
import styles from "./Modal.module.css";

export interface ModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  /** When false the dialog cannot be dismissed (for example while an operation runs). */
  dismissible?: boolean;
  children: ReactNode;
}

/** Accessible modal dialog (Radix). */
export function Modal({
  open,
  onOpenChange,
  title,
  description,
  dismissible = true,
  children,
}: ModalProps) {
  return (
    <Dialog.Root open={open} onOpenChange={(next) => (dismissible || next) && onOpenChange(next)}>
      <Dialog.Portal>
        <Dialog.Overlay className={styles.overlay} />
        <Dialog.Content
          className={styles.content}
          onEscapeKeyDown={(e) => {
            if (!dismissible) e.preventDefault();
          }}
          onPointerDownOutside={(e) => {
            if (!dismissible) e.preventDefault();
          }}
        >
          <Dialog.Title className={styles.title}>{title}</Dialog.Title>
          <Dialog.Description className={styles.description}>{description}</Dialog.Description>
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
