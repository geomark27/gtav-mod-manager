import { render, screen, act } from "@testing-library/react";
import { emitProgress, listenerCount } from "./eventMock";
import { useProgress } from "./progress";
import { makeProgress } from "../features/inventory/testing";

vi.mock("@tauri-apps/api/event", () => import("./eventMock"));

function Probe({ kind }: { kind: "scan" | "baseline" }) {
  const p = useProgress(kind);
  return <p>{p ? `${p.filesSeen}` : "none"}</p>;
}

describe("useProgress", () => {
  it("returns the latest event of its kind and unsubscribes on unmount", async () => {
    const { unmount } = render(<Probe kind="scan" />);
    expect(screen.getByText("none")).toBeInTheDocument();
    await vi.waitFor(() => expect(listenerCount()).toBe(1));

    act(() => emitProgress(makeProgress({ filesSeen: 7 })));
    expect(screen.getByText("7")).toBeInTheDocument();
    act(() => emitProgress(makeProgress({ kind: "baseline", filesSeen: 8 })));
    expect(screen.getByText("7")).toBeInTheDocument();

    unmount();
    expect(listenerCount()).toBe(0);
  });
});
