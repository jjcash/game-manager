// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";

export interface LibraryInfo {
  total: number;
  source: string;
  diagnostics: boolean;
}
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
export async function getLibraryInfo(): Promise<LibraryInfo> {
  const v = await call("library_get_info");
  if (
    !record(v) ||
    !integer(v.total) ||
    v.total > 1_000_000 ||
    typeof v.source !== "string" ||
    typeof v.diagnostics !== "boolean"
  )
    fail();
  return v as unknown as LibraryInfo;
}
export async function getLibraryWindow(
  offset: number,
  limit: number,
): Promise<LibraryWindow> {
  if (
    !integer(offset) ||
    offset > 1_000_000 ||
    !integer(limit) ||
    limit < 1 ||
    limit > 200
  )
    throw new LibraryError("INVALID_REQUEST", "Invalid library window bounds.");
  const v = await call("library_get_window", {
    request: { offset, limit, sort: "title" },
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
