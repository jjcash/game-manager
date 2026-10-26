// SPDX-License-Identifier: GPL-3.0-or-later
import {
  memo,
  Profiler,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import {
  QueryClientProvider,
  useQueries,
  useQuery,
  useQueryClient,
  keepPreviousData,
} from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  getLibraryInfo,
  getLibraryWindow,
  LibraryError,
  type LibraryRow,
  type LibraryCriteria,
  defaultCriteria,
} from "../../ipc/library";
import {
  createLibraryClient,
  MAX_CACHED_PAGES,
  OVERSCAN,
  PAGE_SIZE,
  prunePages,
  ROW_HEIGHT,
} from "./cache";
import { pageGate } from "./requestGate";
import { Thumbnail } from "./Thumbnail";
import { artworkQueue } from "./artwork";
import { useLibraryUI } from "./state";
import { LibraryControls } from "./LibraryControls";
import { Diagnostics } from "./Diagnostics";
import { createMetrics, sample, type Metrics } from "./metrics";

export function LibraryView() {
  const [client] = useState(createLibraryClient);
  const [generation, setGeneration] = useState(0);
  const [metrics, setMetrics] = useState(createMetrics);
  const criteria = useLibraryUI((s) => s.criteria);
  const [applied, setApplied] = useState(criteria);
  const appliedRef = useRef(criteria);
  const pendingCriteria = criteria !== applied;
  useEffect(() => {
    if (criteria === appliedRef.current) return;
    void client.cancelQueries();
    client.clear();
    const timer = setTimeout(
      () => {
        appliedRef.current = criteria;
        setApplied(criteria);
        setMetrics({
          ...createMetrics(),
          criteriaStarted: useLibraryUI.getState().criteriaChangedAt,
        });
        setGeneration((n) => n + 1);
      },
      criteria.search !== appliedRef.current.search ? 150 : 0,
    );
    return () => clearTimeout(timer);
  }, [criteria, client]);
  useEffect(() => () => client.clear(), [client]);
  function refresh() {
    artworkQueue.clear();
    void client.cancelQueries();
    client.clear();
    setMetrics(createMetrics());
    setGeneration((n) => n + 1);
  }
  return (
    <QueryClientProvider client={client}>
      <Library
        generation={generation}
        metrics={metrics}
        refresh={refresh}
        criteria={applied}
        pendingCriteria={pendingCriteria}
      />
    </QueryClientProvider>
  );
}

function Library({
  generation,
  metrics,
  refresh,
  criteria,
  pendingCriteria,
}: {
  generation: number;
  metrics: Metrics;
  refresh: () => void;
  criteria: LibraryCriteria;
  pendingCriteria: boolean;
}) {
  const client = useQueryClient();
  const info = useQuery({
    queryKey: ["info", generation, criteria],
    placeholderData: keepPreviousData, // retain source options; never render placeholder counts/rows
    enabled: !pendingCriteria,
    queryFn: ({ signal }) =>
      pageGate.run(signal, async () => {
        const started = performance.now();
        metrics.active++;
        metrics.maxActive = Math.max(metrics.maxActive, metrics.active);
        try {
          const result = await getLibraryInfo(
            JSON.stringify(criteria) === JSON.stringify(defaultCriteria)
              ? undefined
              : criteria,
          );
          metrics.infoMs = performance.now() - metrics.started;
          if (!signal.aborted && result.diagnostics) {
            sample(metrics.metadataRequests, performance.now() - started);
            if (result.queryMs !== undefined)
              sample(metrics.counts, result.queryMs);
          }
          return result;
        } finally {
          metrics.active--;
        }
      }),
    retry: (count, error) =>
      error instanceof LibraryError && error.code === "NOT_READY" && count < 20,
    retryDelay: 100,
  });
  const scrollRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const selection = useLibraryUI((s) => s.selection);
  const select = useLibraryUI((s) => s.select);
  const lastSource = useRef<string | null>(null);
  const total = info.data?.total ?? 0;
  // Capture the DOM position even if navigation precedes a queued scroll event.
  useLayoutEffect(() => {
    const element = scrollRef.current;
    return () => {
      if (element && useLibraryUI.getState().criteria === criteria)
        useLibraryUI.getState().scroll(element.scrollTop);
    };
  }, [info.isSuccess, total, criteria]);
  // No React Compiler is configured. Read this mutable instance each render;
  // memoized rows receive only immutable DTOs and primitive positions.
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual's documented API is intentionally mutable.
  const virtualizer = useVirtualizer({
    enabled: !pendingCriteria && info.isSuccess && !info.isPlaceholderData,
    count: total,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
    initialOffset: () => useLibraryUI.getState().scrollOffset,
  });
  const positionedCriteria = useRef(criteria);
  useLayoutEffect(() => {
    if (
      info.isSuccess &&
      !info.isPlaceholderData &&
      positionedCriteria.current !== criteria
    ) {
      virtualizer.scrollToOffset(0);
      positionedCriteria.current = criteria;
    }
  }, [criteria, info.isSuccess, info.isPlaceholderData, virtualizer]);
  const rows = virtualizer.getVirtualItems();
  const first = rows[0]?.index ?? 0;
  const last = rows[rows.length - 1]?.index ?? 0;
  const firstPage = Math.floor(first / PAGE_SIZE);
  const lastPage = Math.floor(last / PAGE_SIZE);
  const pageNumbers =
    total && !pendingCriteria && !info.isPlaceholderData
      ? Array.from(
          { length: lastPage - firstPage + 1 },
          (_, i) => firstPage + i,
        )
      : [];
  const pages = useQueries({
    queries: pageNumbers.map((page) => ({
      queryKey: ["page", generation, criteria, page],
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        pageGate.run(signal, async () => {
          const started = performance.now();
          metrics.active++;
          metrics.maxActive = Math.max(metrics.maxActive, metrics.active);
          try {
            const result = await (JSON.stringify(criteria) ===
            JSON.stringify(defaultCriteria)
              ? getLibraryWindow(page * PAGE_SIZE, PAGE_SIZE)
              : getLibraryWindow(page * PAGE_SIZE, PAGE_SIZE, criteria));
            if (
              result.items.length !==
              Math.min(PAGE_SIZE, total - page * PAGE_SIZE)
            )
              throw new LibraryError(
                "CHANGED",
                "Library changed while browsing. Refresh the view.",
              );
            if (!signal.aborted && info.data?.diagnostics) {
              sample(metrics.requests, performance.now() - started);
              sample(metrics.database, result.queryMs);
            }
            return result;
          } finally {
            metrics.active--;
          }
        }),
    })),
  });
  // Inactive pages are evicted by count, rather than relying on a time-only GC.
  useEffect(() => {
    prunePages(client);
    metrics.maxCached = Math.max(
      metrics.maxCached,
      client.getQueryCache().findAll({ queryKey: ["page"] }).length,
    );
  });
  useEffect(() => {
    if (!info.data) return;
    if (
      lastSource.current !== null &&
      lastSource.current !== info.data.source
    ) {
      useLibraryUI.getState().reset();
      virtualizer.scrollToOffset(0);
    }
    lastSource.current = info.data.source;
  }, [info.data, virtualizer]);
  const rowAt = (index: number) =>
    pages[Math.floor(index / PAGE_SIZE) - firstPage]?.data?.items[
      index % PAGE_SIZE
    ];
  const selectedRow = selection ? rowAt(selection.index) : undefined;
  useEffect(() => {
    if (selection && selectedRow && !selection.id)
      select(selectedRow.id, selection.index);
  }, [selection, selectedRow, select]);
  useLayoutEffect(() => {
    metrics.mounted = rows.length;
    metrics.visible = rows.filter(
      (r) =>
        r.end > (virtualizer.scrollOffset ?? 0) &&
        r.start <
          (virtualizer.scrollOffset ?? 0) +
            (virtualizer.scrollRect?.height ?? 0),
    ).length;
    metrics.maxMounted = Math.max(metrics.maxMounted, rows.length);
    if (
      metrics.usableMs === undefined &&
      !pendingCriteria &&
      info.isSuccess &&
      !info.isPlaceholderData &&
      (total === 0 ||
        (metrics.visible > 0 &&
          rows
            .filter(
              (r) =>
                r.end > (virtualizer.scrollOffset ?? 0) &&
                r.start <
                  (virtualizer.scrollOffset ?? 0) +
                    (virtualizer.scrollRect?.height ?? 0),
            )
            .every((r) => rowAt(r.index))))
    ) {
      metrics.usableMs = performance.now() - metrics.started;
      if (metrics.criteriaStarted !== undefined)
        metrics.criteriaVisibleMs = performance.now() - metrics.criteriaStarted;
    }
  });
  function keyboard(e: KeyboardEvent<HTMLDivElement>) {
    const current = selection?.index ?? 0;
    const step = Math.max(
      1,
      Math.floor((scrollRef.current?.clientHeight ?? 380) / ROW_HEIGHT),
    );
    const destinations: Record<string, number> = {
      ArrowDown: selection ? current + 1 : 0,
      ArrowUp: current - 1,
      Home: 0,
      End: total - 1,
      PageDown: current + step,
      PageUp: current - step,
    };
    if (!(e.key in destinations) || !total) return;
    e.preventDefault();
    const index = Math.max(0, Math.min(total - 1, destinations[e.key]));
    select(rowAt(index)?.id ?? null, index);
    virtualizer.scrollToIndex(index, { align: "auto" });
  }
  const error = info.error ?? pages.find((p) => p.error)?.error;
  const busy =
    info.isPending || info.isPlaceholderData || pages.some((p) => p.isFetching);
  const table = (
    <div
      ref={gridRef}
      className="library-table"
      role="grid"
      tabIndex={0}
      aria-label="Game library"
      aria-describedby="library-help"
      aria-activedescendant={
        selection &&
        selectedRow &&
        rows.some((r) => r.index === selection.index)
          ? `game-${selectedRow.id}`
          : undefined
      }
      onKeyDown={keyboard}
      aria-rowcount={total + 1}
      aria-colcount={5}
    >
      <div className="library-columns" role="row" aria-rowindex={1}>
        <span role="columnheader" aria-label="Artwork">
          Art
        </span>
        <span
          role="columnheader"
          aria-sort={
            criteria.sort === "title"
              ? criteria.direction === "asc"
                ? "ascending"
                : "descending"
              : "none"
          }
        >
          Game title{" "}
          {criteria.sort === "title"
            ? criteria.direction === "asc"
              ? "↑"
              : "↓"
            : ""}
        </span>
        <span
          role="columnheader"
          aria-sort={
            criteria.sort === "releaseDate"
              ? criteria.direction === "asc"
                ? "ascending"
                : "descending"
              : "none"
          }
        >
          Release
        </span>
        <span role="columnheader">Library source</span>
        <span role="columnheader">Installation</span>
      </div>
      <div
        ref={scrollRef}
        className="library-scroll"
        role="rowgroup"
        aria-label="Library rows"
        aria-busy={busy}
        onClick={() => gridRef.current?.focus({ preventScroll: true })}
        onScroll={(e) =>
          useLibraryUI.getState().scroll(e.currentTarget.scrollTop)
        }
      >
        <div
          className="library-spacer"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {rows.map((r) => (
            <GameRow
              key={rowAt(r.index)?.id ?? `loading-${r.index}`}
              row={rowAt(r.index)}
              index={r.index}
              top={r.start}
              artworkRevision={generation}
              visible={
                r.end > (virtualizer.scrollOffset ?? 0) &&
                r.start <
                  (virtualizer.scrollOffset ?? 0) +
                    (virtualizer.scrollRect?.height ?? 0)
              }
              selected={
                !!selection &&
                (selection.id
                  ? selection.id === rowAt(r.index)?.id
                  : selection.index === r.index)
              }
              select={select}
            />
          ))}
        </div>
      </div>
    </div>
  );
  return (
    <section
      className="library-panel"
      aria-label="Details View"
      data-query-key={JSON.stringify(criteria)}
      data-query-ready={
        !pendingCriteria &&
        info.isSuccess &&
        !info.isPlaceholderData &&
        !busy &&
        !error
      }
      data-result-count={total}
    >
      <div className="library-toolbar">
        <div>
          <strong>All games</strong>
          <span className="muted">
            {" "}
            {pendingCriteria || info.isPlaceholderData
              ? "Updating criteria…"
              : info.data
                ? `${total.toLocaleString()} titles${info.data.libraryTotal !== undefined ? ` / ${info.data.libraryTotal.toLocaleString()} in library` : ""}`
                : info.isError
                  ? "Library unavailable"
                  : "Connecting to library…"}
          </span>
        </div>
        <button className="check-button" onClick={refresh}>
          Refresh library
        </button>
      </div>
      <LibraryControls sources={info.data?.sources} />
      <p id="library-help" className="library-help">
        Details View · SQLite ordering · Arrow keys to select · Home / End ·
        Page Up / Down
      </p>
      {!pendingCriteria && error && (
        <div className="library-error" role="alert">
          {error instanceof LibraryError ? `${error.code}: ` : ""}
          {error.message}{" "}
          <button
            className="check-button"
            onClick={() => {
              if (info.error) void info.refetch();
              else
                pages
                  .filter((p) => p.isError)
                  .forEach((p) => {
                    void p.refetch();
                  });
            }}
          >
            Retry library
          </button>
        </div>
      )}
      {info.isPending && <p className="library-message">Loading library…</p>}
      {!pendingCriteria &&
        info.isSuccess &&
        !info.isPlaceholderData &&
        total === 0 && (
          <p className="library-message">
            {info.data.libraryTotal === 0 ||
            (info.data.libraryTotal === undefined &&
              JSON.stringify(criteria) === JSON.stringify(defaultCriteria))
              ? "Your library is empty. No games have been added."
              : "No games match these criteria. Clear search or reset filters."}
          </p>
        )}
      {!pendingCriteria &&
        info.isSuccess &&
        !info.isPlaceholderData &&
        total > 0 &&
        (info.data.diagnostics && import.meta.env.DEV ? (
          <Profiler
            id="DetailsView"
            onRender={(_id, _phase, duration) =>
              sample(metrics.commits, duration)
            }
          >
            {table}
          </Profiler>
        ) : (
          table
        ))}
      <div className="library-status" aria-live="polite">
        {pendingCriteria || info.isPlaceholderData
          ? "Updating criteria…"
          : error
            ? "Library unavailable"
            : busy
              ? "Loading visible rows…"
              : selection?.id
                ? "1 game selected"
                : "Ready to browse"}
        <span>100 rows / page · {MAX_CACHED_PAGES} pages maximum</span>
      </div>
      {info.data?.diagnostics && (
        <Diagnostics
          metrics={metrics}
          client={client}
          source={info.data.source}
          total={total}
        />
      )}
    </section>
  );
}
const GameRow = memo(function GameRow({
  row,
  index,
  top,
  visible,
  artworkRevision,
  selected,
  select,
}: {
  row?: LibraryRow;
  index: number;
  top: number;
  visible: boolean;
  artworkRevision: number;
  selected: boolean;
  select: (id: string | null, index: number) => void;
}) {
  return (
    <div
      id={row ? `game-${row.id}` : undefined}
      role="row"
      aria-rowindex={index + 2}
      aria-selected={selected}
      aria-busy={!row}
      className={`game-row library-columns${selected ? " selected" : ""}`}
      style={{ transform: `translateY(${top}px)`, height: ROW_HEIGHT }}
      onClick={() => {
        select(row?.id ?? null, index);
      }}
    >
      <span role="gridcell">
        <Thumbnail id={row?.id} visible={visible} revision={artworkRevision} />
      </span>
      <span role="gridcell" className="game-title" title={row?.title}>
        {row?.title ?? "Loading game…"}
      </span>
      <span role="gridcell">{row ? (row.releaseDate ?? "Unknown") : "—"}</span>
      <span role="gridcell" title={row?.sourceNames}>
        {row ? row.sourceNames || "No source" : "—"}
        {row && row.entryCount > 1 && (
          <small> · {row.entryCount} entries</small>
        )}
      </span>
      <span role="gridcell" className={row?.installed ? "installed" : "muted"}>
        {row ? (row.installed ? "Installed" : "Not installed") : "—"}
        {row && row.installationCount > 1 && (
          <small> · {row.installationCount} copies</small>
        )}
      </span>
    </div>
  );
});
