// SPDX-License-Identifier: GPL-3.0-or-later
import { QueryClient } from "@tanstack/react-query";
export const PAGE_SIZE = 100;
export const MAX_CACHED_PAGES = 6;
export const ROW_HEIGHT = 38;
export const OVERSCAN = 8;
export function createLibraryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: Infinity,
        gcTime: 60_000,
        retry: false,
        refetchOnWindowFocus: false,
        refetchOnReconnect: false,
      },
    },
  });
}
export function prunePages(client: QueryClient) {
  const pages = client.getQueryCache().findAll({ queryKey: ["page"] });
  const inactive = pages
    .filter((q) => q.getObserversCount() === 0)
    .sort((a, b) => a.state.dataUpdatedAt - b.state.dataUpdatedAt);
  let count = pages.length;
  for (const query of inactive) {
    if (count <= MAX_CACHED_PAGES) break;
    client.removeQueries({ queryKey: query.queryKey, exact: true });
    count--;
  }
}
