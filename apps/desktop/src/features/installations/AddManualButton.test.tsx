import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { open } from "@tauri-apps/plugin-dialog";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Mock } from "vitest";
import { AddManualButton } from "./AddManualButton";
import { makeInstallation, renderWithProviders } from "./testing";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
const openMock = open as unknown as Mock;

afterEach(() => {
  clearMocks();
  openMock.mockReset();
});

describe("AddManualButton", () => {
  it("opens a folder picker and registers the chosen folder", async () => {
    openMock.mockResolvedValue("/games/gta");
    const calls: unknown[] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return makeInstallation();
    });
    renderWithProviders(<AddManualButton />);
    await userEvent.click(screen.getByRole("button", { name: "Add installation manually" }));
    expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false });
    await waitFor(() =>
      expect(calls).toContainEqual([
        "register_installation",
        { req: { rootPath: "/games/gta", displayName: null, platform: null } },
      ]),
    );
  });

  it("does nothing when the picker is cancelled", async () => {
    openMock.mockResolvedValue(null);
    const calls: unknown[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      return undefined;
    });
    renderWithProviders(<AddManualButton />);
    await userEvent.click(screen.getByRole("button", { name: "Add installation manually" }));
    await waitFor(() => expect(openMock).toHaveBeenCalled());
    expect(calls).toEqual([]);
  });

  it("shows code, message and hint when registration fails", async () => {
    openMock.mockResolvedValue("/games/bad");
    mockIPC(() => {
      throw {
        code: "INVALID_INPUT",
        message: "This folder is not a complete GTA V installation.",
        hint: "Select the game folder.",
        details: null,
      };
    });
    renderWithProviders(<AddManualButton />);
    await userEvent.click(screen.getByRole("button", { name: "Add installation manually" }));
    expect(await screen.findByText("INVALID_INPUT")).toBeInTheDocument();
    expect(screen.getByText("This folder is not a complete GTA V installation.")).toBeInTheDocument();
    expect(screen.getByText("Select the game folder.")).toBeInTheDocument();
  });
});
