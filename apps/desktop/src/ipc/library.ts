// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";

export interface LibraryInfo {
  total: number;
  source: string;
  diagnostics: boolean;
  libraryTotal: number;
  sources: { id: string; name: string }[];
  queryMs: number;
}
export interface LibraryCriteria {
  search: string;
  installed: "all" | "installed" | "notInstalled";
  sourceId: string | null;
  yearFrom: number | null;
  yearTo: number | null;
  sort: "title" | "releaseDate";
  direction: "asc" | "desc";
}
export const defaultCriteria: LibraryCriteria = {
  search: "",
  installed: "all",
  sourceId: null,
  yearFrom: null,
  yearTo: null,
  sort: "title",
  direction: "asc",
};
export interface LibraryRow {
  id: string;
  title: string;
  releaseDate: string | null;
  sourceNames: string;
  entryCount: number;
  installationCount: number;
  installed: boolean;
}
export interface LibraryWindow {
  offset: number;
  items: LibraryRow[];
  queryMs: number;
}
export class LibraryError extends Error {
  constructor(
    public code: string,
    message: string,
  ) {
    super(message);
  }
}
const record = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null;
const integer = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
function validateCriteria(criteria: LibraryCriteria) {
  if (
    [criteria.yearFrom, criteria.yearTo].some(
      (y) => y !== null && (!integer(y) || y < 1 || y > 9999),
    ) ||
    (criteria.yearFrom !== null &&
      criteria.yearTo !== null &&
      criteria.yearFrom > criteria.yearTo)
  )
    throw new LibraryError(
      "INVALID_REQUEST",
      "Use release years 1-9999, with from no later than to.",
    );
  if (
    criteria.search.includes("\0") ||
    Array.from(criteria.search).length > 200
  )
    throw new LibraryError(
      "INVALID_REQUEST",
      "Search must be at most 200 characters and contain no NUL.",
    );
}
function fail(): never {
  throw new LibraryError(
    "INVALID_RESPONSE",
    "Unexpected library response. Restart Game Manager.",
  );
}
async function call(
  command: string,
  args?: Record<string, unknown>,
): Promise<unknown> {
  try {
    return await invoke<unknown>(command, args);
  } catch (e) {
    if (
      record(e) &&
      typeof e.code === "string" &&
      typeof e.message === "string"
    )
      throw new LibraryError(e.code, e.message);
    throw new LibraryError(
      "CONNECTION",
      "Could not reach the library. Open the Game Manager desktop window and retry.",
    );
  }
}
export async function getLibraryInfo(
  criteria?: LibraryCriteria,
): Promise<LibraryInfo> {
  validateCriteria(criteria ?? defaultCriteria);
  const v = await call("library_get_info", {
    criteria: criteria ?? defaultCriteria,
  });
  if (
    !record(v) ||
    !integer(v.total) ||
    v.total > 1_000_000 ||
    typeof v.source !== "string" ||
    typeof v.diagnostics !== "boolean"
  )
    fail();
  if (!integer(v.libraryTotal) || v.libraryTotal < v.total) fail();
  if (
    typeof v.queryMs !== "number" ||
    !Number.isFinite(v.queryMs) ||
    v.queryMs < 0
  )
    fail();
  if (
    !Array.isArray(v.sources) ||
    v.sources.length > 100 ||
    v.sources.some(
      (s) =>
        !record(s) ||
        typeof s.id !== "string" ||
        !s.id ||
        typeof s.name !== "string",
    )
  )
    fail();
  return v as unknown as LibraryInfo;
}
export async function getLibraryWindow(
  offset: number,
  limit: number,
  criteria?: LibraryCriteria,
): Promise<LibraryWindow> {
  if (
    !integer(offset) ||
    offset > 1_000_000 ||
    !integer(limit) ||
    limit < 1 ||
    limit > 200
  )
    throw new LibraryError("INVALID_REQUEST", "Invalid library window bounds.");
  validateCriteria(criteria ?? defaultCriteria);
  const v = await call("library_query", {
    request: { offset, limit, criteria: criteria ?? defaultCriteria },
  });
  if (
    !record(v) ||
    v.offset !== offset ||
    !Array.isArray(v.items) ||
    v.items.length > limit ||
    typeof v.queryMs !== "number" ||
    !Number.isFinite(v.queryMs) ||
    v.queryMs < 0
  )
    fail();
  const ids = new Set<string>();
  for (const row of v.items) {
    if (
      !record(row) ||
      typeof row.id !== "string" ||
      !row.id ||
      ids.has(row.id) ||
      typeof row.title !== "string" ||
      !(row.releaseDate === null || typeof row.releaseDate === "string") ||
      typeof row.sourceNames !== "string" ||
      !integer(row.entryCount) ||
      !integer(row.installationCount) ||
      typeof row.installed !== "boolean"
    )
      fail();
    ids.add(row.id);
  }
  return v as unknown as LibraryWindow;
}
