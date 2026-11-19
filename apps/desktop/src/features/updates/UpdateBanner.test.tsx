import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { UpdateBanner, BUSY_REASON } from "./UpdateBanner";
import { UpdatesProvider, useUpdater } from "./useUpdater";

vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn(async () => {}) }));

const checkMock = vi.mocked(check);
const relaunchMock = vi.mocked(relaunch);

function fakeUpdate(
  version: string,
  body: string | undefined,
  downloadAndInstall: (onEvent?: (event: DownloadEvent) => void) => Promise<void>,
): Update {
  return { version, body, downloadAndInstall } as unknown as Update;
}

function idle() {
  mockIPC((cmd) => (cmd === "operation_status" ? { busy: false, kind: null } : undefined));
}

function Harness() {
  const { checkNow } = useUpdater();
  return (
    <>
      <button
        onClick={() => {
          void checkNow();
        }}
      >
        Run check
      </button>
      <UpdateBanner />
    </>
  );
}

function renderBanner() {
  return render(
    <UpdatesProvider>
      <Harness />
    </UpdatesProvider>,
  );
}

async function showAvailableUpdate(update: Update) {
  checkMock.mockResolvedValue(update);
  const user = userEvent.setup();
  renderBanner();
  await user.click(screen.getByRole("button", { name: "Run check" }));
  expect(await screen.findByText(`Version ${update.version} is available.`)).toBeInTheDocument();
  return user;
}

beforeEach(() => {
  idle();
});

afterEach(() => {
  clearMocks();
});

describe("UpdateBanner", () => {
  it("appears when an update is available, with plain-text release notes", async () => {
    await showAvailableUpdate(
      fakeUpdate("0.2.0", "Fixes a scan bug.", vi.fn(async () => {})),
    );
    expect(screen.getByText("Fixes a scan bug.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Update now" })).toBeEnabled();
  });

  it("disables Update now and explains why while an operation is busy", async () => {
    mockIPC((cmd) =>
      cmd === "operation_status" ? { busy: true, kind: "scan" } : undefined,
    );
    await showAvailableUpdate(fakeUpdate("0.2.0", undefined, vi.fn(async () => {})));
    expect(await screen.findByText(BUSY_REASON)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Update now" })).toBeDisabled();
  });

  it("shows download progress in bytes", async () => {
    let emit: ((event: DownloadEvent) => void) | undefined;
    let finish: (() => void) | undefined;
    const downloadAndInstall = vi.fn((onEvent?: (event: DownloadEvent) => void) => {
      emit = onEvent;
      return new Promise<void>((resolve) => {
        finish = resolve;
      });
    });
    const user = await showAvailableUpdate(
      fakeUpdate("0.2.0", undefined, downloadAndInstall),
    );

    await user.click(screen.getByRole("button", { name: "Update now" }));
    await waitFor(() => expect(downloadAndInstall).toHaveBeenCalled());
    act(() => {
      emit?.({ event: "Started", data: { contentLength: 1000 } });
      emit?.({ event: "Progress", data: { chunkLength: 500 } });
    });

    const bar = await screen.findByRole("progressbar", { name: "Update download" });
    expect(bar).toHaveAttribute("aria-valuenow", "50");
    expect(screen.getByText("500 B of 1000 B")).toBeInTheDocument();

    await act(async () => {
      finish?.();
    });
  });

  it("relaunches the app after installing", async () => {
    const downloadAndInstall = vi.fn(async () => {});
    await showAvailableUpdate(fakeUpdate("0.2.0", undefined, downloadAndInstall));
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Update now" }));

    await waitFor(() => expect(downloadAndInstall).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(relaunchMock).toHaveBeenCalledTimes(1));
  });

  it("hides the banner with Later until the next check", async () => {
    const user = await showAvailableUpdate(
      fakeUpdate("0.2.0", undefined, vi.fn(async () => {})),
    );
    await user.click(screen.getByRole("button", { name: "Later" }));
    expect(screen.queryByText("Version 0.2.0 is available.")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Run check" }));
    expect(await screen.findByText("Version 0.2.0 is available.")).toBeInTheDocument();
  });
});
