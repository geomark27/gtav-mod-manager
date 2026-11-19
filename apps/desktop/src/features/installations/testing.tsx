import { QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import { MemoryRouter } from "react-router";
import type { InstallationDto } from "../../bindings/InstallationDto";
import { createQueryClient } from "../../lib/queryClient";

/** Builds an installation DTO for tests. */
export function makeInstallation(over: Partial<InstallationDto> = {}): InstallationDto {
  return {
    id: "0192f0e8-0000-7000-8000-000000000001",
    edition: "legacy",
    platform: "steam",
    displayName: "GTA V Legacy (Steam)",
    rootPath: "/games/gta",
    exeVersion: "1.0.3179.0",
    status: "ok",
    baseline: "none",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...over,
  };
}

/** Renders inside a fresh query client and router. */
export function renderWithProviders(ui: ReactElement) {
  return render(
    <QueryClientProvider client={createQueryClient()}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>,
  );
}
