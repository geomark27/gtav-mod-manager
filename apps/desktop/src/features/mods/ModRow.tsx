import * as Tooltip from "@radix-ui/react-tooltip";
import { TriangleAlert } from "lucide-react";
import { useState } from "react";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import type { ModDto } from "../../bindings/ModDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { ModSwitch } from "../../components/ModSwitch";
import { formatBytes } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import { MOD_STATE_INFO, stateProblem, switchState } from "./modLabels";
import styles from "./Mods.module.css";
import { useSetModEnabled } from "./useMods";
import { usePreviewModToggle } from "../dependencies/useDependencies";
import { ToggleClosureDialog } from "../dependencies/ToggleClosureDialog";

export interface ModRowProps {
  installationId: string;
  mod: ModDto;
  /** Why writes are blocked right now, or `null`. */
  blockedReason: string | null;
  checking: boolean;
  onOpen: (mod: ModDto) => void;
  onCheckAgain: () => void;
}

/** One mod: details link, state, conflicts and its switch. */
export function ModRow({
  installationId,
  mod,
  blockedReason,
  checking,
  onOpen,
  onCheckAgain,
}: ModRowProps) {
  const toggle = useSetModEnabled(installationId);
  const preview = usePreviewModToggle();
  const [confirm, setConfirm] = useState<{ enable: boolean; preview: PlanPreviewDto } | null>(null);
  const info = MOD_STATE_INFO[mod.state];
  const problem = stateProblem(mod.state);
  const conflicts = mod.conflictsWith.map((c) => c.name).join(", ");

  return (
    <li className={styles.row}>
      <div className={styles.rowMain}>
        <button type="button" className={styles.nameButton} onClick={() => onOpen(mod)}>
          {mod.name}
        </button>
        <span className={styles.muted}>
          {mod.entryCount} {mod.entryCount === 1 ? "file" : "files"}
        </span>
        <span className={styles.muted}>{formatBytes(mod.totalSize)}</span>
        {mod.dependsOnCount > 0 ? (
          <span className={styles.requires}>requires {mod.dependsOnCount}</span>
        ) : null}
        <Badge tone={info.tone}>{info.label}</Badge>
        {mod.conflictsWith.length > 0 ? (
          <Tooltip.Root>
            <Tooltip.Trigger asChild>
              <span
                className={styles.conflict}
                tabIndex={0}
                role="img"
                aria-label={`Conflicts with ${conflicts}`}
              >
                <TriangleAlert size={16} aria-hidden="true" />
              </span>
            </Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content className={styles.tooltip} sideOffset={6}>
                Shares files with: {conflicts}
              </Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>
        ) : null}
        <span className={styles.switchSlot}>
          <ModSwitch
            label={mod.name}
            state={switchState(mod.state, toggle.isPending || preview.isPending, blockedReason)}
            blockedReason={blockedReason ?? undefined}
            onRequestChange={(next) => {
              toggle.reset();
              // Dependents are not part of the list data, so the closure is always previewed.
              preview.mutate(
                { modId: mod.id, enabled: next },
                {
                  onSuccess: (p) => {
                    if (p.modsAffected.length > 1) setConfirm({ enable: next, preview: p });
                    else toggle.mutate({ modId: mod.id, enabled: next });
                  },
                },
              );
            }}
          />
        </span>
      </div>
      {problem ? (
        <div className={styles.problem}>
          <span>{problem}</span>
          {mod.state === "drifted" ? (
            <Button loading={checking} onClick={onCheckAgain}>
              Check again
            </Button>
          ) : null}
        </div>
      ) : null}
      {preview.error ? <ErrorMessage error={preview.error} /> : null}
      {confirm === null && toggle.error ? <ErrorMessage error={toggle.error} /> : null}
      {confirm ? (
        <ToggleClosureDialog
          modName={mod.name}
          enable={confirm.enable}
          preview={confirm.preview}
          applying={toggle.isPending}
          error={toggle.error}
          onCancel={() => {
            toggle.reset();
            setConfirm(null);
          }}
          onApply={() =>
            toggle.mutate(
              { modId: mod.id, enabled: confirm.enable, withDependencies: true },
              { onSuccess: () => setConfirm(null) },
            )
          }
        />
      ) : null}
    </li>
  );
}
