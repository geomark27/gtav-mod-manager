import * as Switch from "@radix-ui/react-switch";
import * as Tooltip from "@radix-ui/react-tooltip";
import { useId } from "react";
import styles from "./ModSwitch.module.css";

export type ModSwitchState = "off" | "on" | "pending" | "blocked" | "error";

export interface ModSwitchProps {
  state: ModSwitchState;
  label: string;
  blockedReason?: string;
  onRequestChange: (next: boolean) => void;
}

/**
 * Mod enable/disable switch (architecture D-11).
 *
 * It never changes its own visual state: `checked` derives only from `state`, and a click only
 * requests a change through `onRequestChange`. The parent reports the outcome via `state`.
 */
export function ModSwitch({ state, label, blockedReason, onRequestChange }: ModSwitchProps) {
  const reasonId = useId();
  const checked = state === "on";
  const pending = state === "pending";
  const blocked = state === "blocked";
  const error = state === "error";
  const reason = blocked ? (blockedReason ?? "Blocked") : undefined;

  const control = (
    <Switch.Root
      className={styles.root}
      checked={checked}
      disabled={pending || blocked}
      aria-label={label}
      aria-busy={pending ? "true" : undefined}
      aria-describedby={reason ? reasonId : undefined}
      data-state-name={state}
      onCheckedChange={(next) => {
        if (pending || blocked) return;
        onRequestChange(next);
      }}
    >
      <Switch.Thumb className={styles.thumb} />
    </Switch.Root>
  );

  return (
    <span className={styles.wrapper}>
      {blocked ? (
        <Tooltip.Provider delayDuration={200}>
          <Tooltip.Root>
            <Tooltip.Trigger asChild>
              <span className={styles.trigger} tabIndex={0}>
                {control}
              </span>
            </Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content className={styles.tooltip} sideOffset={6}>
                {reason}
                <Tooltip.Arrow className={styles.arrow} />
              </Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>
        </Tooltip.Provider>
      ) : (
        control
      )}
      {pending ? <span className={styles.spinner} aria-hidden="true" /> : null}
      {error ? (
        <span className={styles.errorIndicator} role="img" aria-label="Last change failed">
          !
        </span>
      ) : null}
      {reason ? (
        <span id={reasonId} className={styles.srOnly}>
          {reason}
        </span>
      ) : null}
    </span>
  );
}
