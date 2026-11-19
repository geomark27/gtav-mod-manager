import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { check } from "@tauri-apps/plugin-updater";
import { act, render } from "@testing-library/react";
import { UpdatesProvider, AUTO_CHECK_DELAY_MS } from "./useUpdater";

vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn(async () => {}) }));

const checkMock = vi.mocked(check);

afterEach(() => {
  clearMocks();
  vi.useRealTimers();
});

describe("useUpdater", () => {
  it("does not check automatically in development", async () => {
    expect(import.meta.env.DEV).toBe(true);
    vi.useFakeTimers();
    mockIPC((cmd) => (cmd === "operation_status" ? { busy: false, kind: null } : undefined));

    render(
      <UpdatesProvider>
        <p>content</p>
      </UpdatesProvider>,
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(AUTO_CHECK_DELAY_MS * 3);
    });
    expect(checkMock).not.toHaveBeenCalled();
  });
});
