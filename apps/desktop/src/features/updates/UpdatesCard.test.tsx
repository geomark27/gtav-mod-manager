import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { UpdatesCard } from "./UpdatesCard";
import { UpdatesProvider } from "./useUpdater";

vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "0.1.0") }));

const checkMock = vi.mocked(check);
const getVersionMock = vi.mocked(getVersion);

function renderCard() {
  mockIPC((cmd) => (cmd === "operation_status" ? { busy: false, kind: null } : undefined));
  return render(
    <UpdatesProvider>
      <UpdatesCard />
    </UpdatesProvider>,
  );
}

beforeEach(() => {
  getVersionMock.mockResolvedValue("0.1.0");
});

afterEach(() => {
  clearMocks();
});

describe("UpdatesCard", () => {
  it("shows the installed version and that it is the latest one", async () => {
    checkMock.mockResolvedValue(null);
    const user = userEvent.setup();
    renderCard();

    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("You have the latest version.")).toBeInTheDocument();
  });

  it("reports an available update for the shared banner", async () => {
    checkMock.mockResolvedValue({ version: "0.2.0", body: null } as unknown as Update);
    const user = userEvent.setup();
    renderCard();

    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(
      await screen.findByText(/Version 0\.2\.0 is available\. Use the banner/),
    ).toBeInTheDocument();
  });

  it("shows manual-check errors with their message", async () => {
    checkMock.mockRejectedValue(new Error("The update server is unreachable."));
    const user = userEvent.setup();
    renderCard();

    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The update server is unreachable.",
    );
  });
});
