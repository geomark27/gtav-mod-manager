import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { screen } from "@testing-library/react";
import { renderWithProviders } from "../features/installations/testing";
import { DashboardPage } from "./DashboardPage";

afterEach(() => clearMocks());

describe("DashboardPage", () => {
  it("links to Installations when there is no active installation", async () => {
    mockIPC((cmd) => (cmd === "get_selected_installation" ? null : undefined));
    renderWithProviders(<DashboardPage />);
    expect(await screen.findByRole("link", { name: "Go to Installations" })).toHaveAttribute(
      "href",
      "/installations",
    );
  });
});
