import { keepPreviousData, useQuery } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { TransactionPageDto } from "../../bindings/TransactionPageDto";
import { commands } from "../../lib/ipc";

/** Transactions per page. */
export const PAGE_SIZE = 20;

/** One page (0-based) of the history; the previous page stays visible while the next loads. */
export function useTransactions(installationId: string, page: number) {
  return useQuery<TransactionPageDto, AppError>({
    queryKey: ["transactions", installationId, page],
    queryFn: () => commands.listTransactions(installationId, PAGE_SIZE, page * PAGE_SIZE),
    placeholderData: keepPreviousData,
    retry: false,
  });
}
