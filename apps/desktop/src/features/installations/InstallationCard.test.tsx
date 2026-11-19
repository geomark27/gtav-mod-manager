import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { InstallationStatus } from "../../bindings/InstallationStatus";
import { InstallationCard } from "./InstallationCard";
import { makeInstallation, renderWithProviders } from "./testing";

afterEach(() => clearMocks());

describe("InstallationCard", () => {
  it.each<[InstallationStatus, string]>([
    ["ok", "Ready"],
    ["incomplete", "Incomplete"],
    ["missing", "Not found"],
    ["not_writable", "Read-only"],
  ])("shows the %s status as %s", (status, label) => {
    mockIPC(() => undefined);
    renderWithProviders(
      <InstallationCard installation={makeInstallation({ status })} isActive={false} />,
    );
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it("shows name, edition, platform, path and version", () => {
    mockIPC(() => undefined);
    renderWithProviders(<InstallationCard installation={makeInstallation()} isActive={false} />);
    expect(screen.getByText("GTA V Legacy (Steam)")).toBeInTheDocument();
    expect(screen.getByText("Legacy")).toBeInTheDocument();
    expect(screen.getByText("Steam")).toBeInTheDocument();
    expect(screen.getByText("/games/gta")).toBeInTheDocument();
    expect(screen.getByText("1.0.3179.0")).toBeInTheDocument();
  });

  it("shows Unknown version when the version is null", () => {
    mockIPC(() => undefined);
    renderWithProviders(
      <InstallationCard installation={makeInstallation({ exeVersion: null })} isActive={false} />,
    );
    expect(screen.getByText("Unknown version")).toBeInTheDocument();
  });

  it("explains read-only folders", () => {
    mockIPC(() => undefined);
    renderWithProviders(
      <InstallationCard
        installation={makeInstallation({ status: "not_writable" })}
        isActive={false}
      />,
    );
    expect(
      screen.getByText(
        "This folder is protected. The app can inspect it but cannot change files without administrator rights, which this version does not use.",
      ),
    ).toBeInTheDocument();
  });

  it("hides Set active and shows the Active badge for the active installation", () => {
    mockIPC(() => undefined);
    renderWithProviders(<InstallationCard installation={makeInstallation()} isActive />);
    expect(screen.getByText("Active")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /set .* as active/i })).not.toBeInTheDocument();
  });

  it("calls select_installation on Set active", async () => {
    const calls: unknown[] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return undefined;
    });
    renderWithProviders(<InstallationCard installation={makeInstallation()} isActive={false} />);
    await userEvent.click(screen.getByRole("button", { name: /set .* as active/i }));
    await waitFor(() =>
      expect(calls).toContainEqual([
        "select_installation",
        { id: "0192f0e8-0000-7000-8000-000000000001" },
      ]),
    );
  });

  it("calls refresh_installation on Refresh", async () => {
    const calls: unknown[] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return makeInstallation({ status: "incomplete" });
    });
    renderWithProviders(<InstallationCard installation={makeInstallation()} isActive={false} />);
    await userEvent.click(screen.getByRole("button", { name: /refresh/i }));
    await waitFor(() =>
      expect(calls).toContainEqual([
        "refresh_installation",
        { id: "0192f0e8-0000-7000-8000-000000000001" },
      ]),
    );
  });

  it("shows a refresh error inline", async () => {
    mockIPC(() => {
      throw { code: "NOT_FOUND", message: "The installation was not found.", hint: null, details: null };
    });
    renderWithProviders(<InstallationCard installation={makeInstallation()} isActive={false} />);
    await userEvent.click(screen.getByRole("button", { name: /refresh/i }));
    expect(await screen.findByText("NOT_FOUND")).toBeInTheDocument();
    expect(screen.getByText("The installation was not found.")).toBeInTheDocument();
  });

  it("renames inline: save sends the name, cancel discards", async () => {
    const calls: unknown[] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return makeInstallation({ displayName: "Main" });
    });
    renderWithProviders(<InstallationCard installation={makeInstallation()} isActive={false} />);
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    const input = screen.getByRole("textbox", { name: "Installation name" });
    await userEvent.clear(input);
    await userEvent.type(input, "Main");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(calls).toContainEqual([
        "rename_installation",
        { id: "0192f0e8-0000-7000-8000-000000000001", displayName: "Main" },
      ]),
    );
    await waitFor(() => expect(screen.queryByRole("textbox")).not.toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("keeps the editor open and shows the error when rename fails", async () => {
    mockIPC(() => {
      throw {
        code: "INVALID_INPUT",
        message: "The name must be between 1 and 80 characters.",
        hint: null,
        details: null,
      };
    });
    renderWithProviders(<InstallationCard installation={makeInstallation()} isActive={false} />);
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText("The name must be between 1 and 80 characters."),
    ).toBeInTheDocument();
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });
});
