import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import type { AppError } from "../../bindings/AppError";
import type { ImportPreviewDto } from "../../bindings/ImportPreviewDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { commands, toAppError } from "../../lib/ipc";
import { useProgress } from "../../lib/progress";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "../mods/Mods.module.css";
import { ImportDialog } from "./ImportDialog";
import { useStageImport } from "./useImport";

export interface ImportZipButtonProps {
  installationId: string;
  /** Why writes are blocked right now, or `null`. */
  blockedReason: string | null;
}

/** "Import ZIP…": picks an archive, stages it (with progress and Cancel) and opens the import dialog. */
export function ImportZipButton({ installationId, blockedReason }: ImportZipButtonProps) {
  const stage = useStageImport(installationId);
  const [preview, setPreview] = useState<ImportPreviewDto | null>(null);
  const [pickError, setPickError] = useState<AppError | null>(null);
  const [picking, setPicking] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  const pick = async () => {
    setPickError(null);
    stage.reset();
    setPicking(true);
    try {
      const file = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "ZIP archive", extensions: ["zip"] }],
      });
      if (typeof file === "string") {
        setCancelling(false);
        stage.mutate(file, { onSuccess: setPreview });
      }
    } catch (e) {
      setPickError(toAppError(e));
    } finally {
      setPicking(false);
    }
  };

  const cancelStaging = () => {
    setCancelling(true);
    commands.cancelOperation().catch((e: unknown) => setPickError(toAppError(e)));
  };

  // A user-requested cancellation is not an error.
  const stageError = stage.error && stage.error.code !== "CANCELLED" ? stage.error : null;
  const error = pickError ?? stageError;

  return (
    <>
      <Button
        variant="primary"
        loading={picking}
        disabled={blockedReason !== null || stage.isPending}
        title={blockedReason ?? undefined}
        onClick={() => void pick()}
      >
        Import ZIP…
      </Button>
      {error ? <ErrorMessage error={error} /> : null}
      {stage.isPending ? <StagingDialog cancelling={cancelling} onCancel={cancelStaging} /> : null}
      {preview ? (
        <ImportDialog
          key={preview.stagingId}
          installationId={installationId}
          initial={preview}
          blockedReason={blockedReason}
          onClose={() => setPreview(null)}
        />
      ) : null}
    </>
  );
}

function StagingDialog({ cancelling, onCancel }: { cancelling: boolean; onCancel: () => void }) {
  const progress = useProgress("import");
  return (
    <Modal
      open
      onOpenChange={() => undefined}
      dismissible={false}
      title="Reading archive"
      description="The archive is being checked. Nothing in the game folder is changed."
    >
      <div className={styles.stack}>
        <p role="status">
          {progress && progress.filesSeen > 0
            ? `${progress.filesSeen} files read…`
            : "Reading the archive…"}
        </p>
        <div className={styles.actions}>
          <Button disabled={cancelling} onClick={onCancel}>
            Cancel
          </Button>
        </div>
      </div>
    </Modal>
  );
}
