import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import type { AppError } from "../../bindings/AppError";
import { Button } from "../../components/Button";
import { toAppError } from "../../lib/ipc";
import { ErrorMessage } from "./ErrorMessage";
import { useRegisterInstallation } from "./useInstallations";

/** Lets the user pick a folder and registers it. */
export function AddManualButton() {
  const register = useRegisterInstallation();
  const [pickError, setPickError] = useState<AppError | null>(null);
  const [picking, setPicking] = useState(false);

  const pick = async () => {
    setPickError(null);
    register.reset();
    setPicking(true);
    try {
      const folder = await open({ directory: true, multiple: false });
      if (typeof folder === "string") {
        register.mutate({ rootPath: folder, displayName: null, platform: null });
      }
    } catch (e) {
      setPickError(toAppError(e));
    } finally {
      setPicking(false);
    }
  };

  const error = pickError ?? register.error;
  return (
    <div>
      <Button variant="primary" loading={picking || register.isPending} onClick={() => void pick()}>
        Add installation manually
      </Button>
      {error ? <ErrorMessage error={error} /> : null}
    </div>
  );
}
