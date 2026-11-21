import { QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { createQueryClient } from "../lib/queryClient";
import { AppRoutes } from "../routes";
import { ONLINE_NOTICE } from "./AppShell";

function renderAt(path: string) {
  mockIPC(() => {
    throw "backend not available in this test";
  });
  return render(
    <QueryClientProvider client={createQueryClient()}>
      <MemoryRouter initialEntries={[path]}>
        <AppRoutes />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => clearMocks());

describe("AppShell", () => {
  it("renders the 7 navigation links", () => {
    renderAt("/");
    const nav = screen.getByRole("navigation", { name: "Main" });
    for (const name of [
      "Dashboard",
      "Mods",
      "Review",
      "Profiles",
      "Activity",
      "Installations",
      "Settings",
    ]) {
      expect(nav).toContainElement(screen.getByRole("link", { name }));
    }
    expect(screen.getAllByRole("link")).toHaveLength(7);
  });

  it("shows the installation selector at the top of the sidebar", () => {
    renderAt("/");
    expect(screen.getByText("Active installation")).toBeInTheDocument();
  });

  it("shows the persistent online notice", () => {
    renderAt("/");
    const note = screen.getByRole("note");
    expect(note).toHaveTextContent(ONLINE_NOTICE);
    expect(ONLINE_NOTICE).toBe(
      "Mods managed here are for story mode. Disabling mods does not make an installation safe for GTA Online or Red Dead Online.",
    );
  });

  it("shows the dashboard with the System status card", () => {
    renderAt("/");
    expect(screen.getByRole("heading", { level: 1, name: "Dashboard" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "System status" })).toBeInTheDocument();
    expect(screen.queryByText(/Planned for milestone/)).not.toBeInTheDocument();
  });

  it("navigates to /mods and marks the link as current", async () => {
    renderAt("/mods");
    expect(await screen.findByRole("heading", { level: 1, name: "Mods" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Mods" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Dashboard" })).not.toHaveAttribute("aria-current");
  });

  it("shows the not found page for unknown routes", () => {
    renderAt("/nope");
    expect(screen.getByRole("heading", { level: 1, name: "Page not found" })).toBeInTheDocument();
  });
});
