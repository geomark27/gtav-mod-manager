import { QueryClientProvider } from "@tanstack/react-query";
import { useState } from "react";
import { HashRouter } from "react-router";
import { useDataChangedRefresh } from "./lib/dataChanged";
import { createQueryClient } from "./lib/queryClient";
import { AppRoutes } from "./routes";

/** Application root: query client, hash router and routes. */
export function App() {
  const [queryClient] = useState(createQueryClient);
  useDataChangedRefresh(queryClient);
  return (
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <AppRoutes />
      </HashRouter>
    </QueryClientProvider>
  );
}
