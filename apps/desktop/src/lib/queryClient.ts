import { QueryClient } from "@tanstack/react-query";

/** Creates the application's TanStack Query client. Mutations are never retried. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { refetchOnWindowFocus: false },
      mutations: { retry: false },
    },
  });
}
