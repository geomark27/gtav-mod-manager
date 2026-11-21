import { QueryClient } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { emitProgress, listenerCount } from "./eventMock";
import { useDataChangedRefresh } from "./dataChanged";

vi.mock("@tauri-apps/api/event", () => import("./eventMock"));

describe("useDataChangedRefresh", () => {
  it("invalidates every query on the event and unsubscribes on unmount", async () => {
    const client = new QueryClient();
    const invalidate = vi.spyOn(client, "invalidateQueries");
    const { unmount } = renderHook(() => useDataChangedRefresh(client));
    await waitFor(() => expect(listenerCount()).toBe(1));
    // The mock delivers any event to every subscriber.
    emitProgress({} as never);
    expect(invalidate).toHaveBeenCalledWith();
    unmount();
    expect(listenerCount()).toBe(0);
  });
});
