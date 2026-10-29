// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";
import { LibraryError } from "./library";

export interface Metadata {
  title: string;
  releaseDate: string | null;
  genre: string | null;
  developer: string | null;
  favorite: boolean;
}
export interface InstallationInput {
  rootPathRef: string;
  platform: string;
  version: string | null;
  detectionState: "present" | "missing" | "unknown";
}
export interface Entry {
  id: string;
  sourceId: string;
  sourceName: string;
  sourceType: string;
  completionStatus: string;
}
export interface Installation extends InstallationInput {
  id: string;
  entryId: string;
  stateOrigin: string;
  groupId: string | null;
}
export interface InstallationGroup {
  id: string;
  displayName: string;
}
export interface Details {
  id: string;
  revision: number;
  origin: string;
  metadata: Metadata;
  overriddenFields: string[];
  entries: Entry[];
  installations: Installation[];
  groups: InstallationGroup[];
}
export type Action =
  | { kind: "metadata"; metadata: Metadata }
  | { kind: "addEntry"; sourceName: string }
  | { kind: "updateEntry"; entryId: string; completionStatus: string }
  | { kind: "removeEntry"; entryId: string }
  | {
      kind: "saveInstallation";
      entryId: string;
      installationId: string | null;
      installation: InstallationInput;
    }
  | { kind: "removeInstallation"; installationId: string }
  | { kind: "saveGroup"; groupId: string | null; displayName: string }
  | { kind: "assignGroup"; installationId: string; groupId: string | null }
  | { kind: "removeGroup"; groupId: string }
  | { kind: "removeGame" };

export function validateMetadata(v: Metadata) {
  if (
    !v.title.trim() ||
    Array.from(v.title.trim()).length > 200 ||
    /\p{Cc}/u.test(v.title)
  )
    throw new LibraryError(
      "INVALID_REQUEST",
      "Enter a title of 1–200 characters without control characters.",
    );
  if (
    v.releaseDate &&
    (!/^\d{4}-\d{2}-\d{2}$/.test(v.releaseDate) ||
      v.releaseDate.startsWith("0000") ||
      !Number.isFinite(Date.parse(v.releaseDate)) ||
      new Date(v.releaseDate).toISOString().slice(0, 10) !== v.releaseDate)
  )
    throw new LibraryError(
      "INVALID_REQUEST",
      "Enter a valid release date as YYYY-MM-DD.",
    );
  for (const value of [v.genre, v.developer])
    if (value && (Array.from(value).length > 200 || /\p{Cc}/u.test(value)))
      throw new LibraryError(
        "INVALID_REQUEST",
        "Metadata must be at most 200 characters without control characters.",
      );
}
export function validateInstallation(v: InstallationInput) {
  if (
    !v.rootPathRef.trim() ||
    v.rootPathRef.length > 2048 ||
    !v.platform.trim() ||
    v.platform.length > 80 ||
    (v.version?.length ?? 0) > 200 ||
    [v.rootPathRef, v.platform, v.version ?? ""].some((s) =>
      /\p{Cc}/u.test(s),
    ) ||
    !["present", "missing", "unknown"].includes(v.detectionState)
  )
    throw new LibraryError(
      "INVALID_REQUEST",
      "Enter a location and platform without control characters, within the field limits.",
    );
}
async function call<T>(
  name: string,
  args: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (e) {
    if (
      typeof e === "object" &&
      e !== null &&
      "code" in e &&
      "message" in e &&
      typeof e.code === "string" &&
      typeof e.message === "string"
    )
      throw new LibraryError(e.code, e.message);
    throw new LibraryError(
      "CONNECTION",
      "Could not reach the library. Retry in the desktop application.",
    );
  }
}
export async function getManual(gameId: string) {
  const value = await call<Details>("manual_get", { gameId });
  if (
    !value ||
    value.id !== gameId ||
    !Number.isSafeInteger(value.revision) ||
    value.revision < 0 ||
    !value.metadata ||
    typeof value.metadata.title !== "string" ||
    !["manual", "legacy"].includes(value.origin) ||
    typeof value.metadata.favorite !== "boolean" ||
    [
      value.metadata.releaseDate,
      value.metadata.genre,
      value.metadata.developer,
    ].some((v) => v !== null && typeof v !== "string") ||
    !Array.isArray(value.entries) ||
    value.entries.length > 100 ||
    !Array.isArray(value.installations) ||
    value.installations.length > 100 ||
    !Array.isArray(value.groups) ||
    value.groups.length > 100 ||
    value.groups.some(
      (g) =>
        !g ||
        typeof g.id !== "string" ||
        !g.id ||
        typeof g.displayName !== "string" ||
        !g.displayName.trim() ||
        Array.from(g.displayName).length > 80 ||
        /\p{Cc}/u.test(g.displayName),
    ) ||
    new Set(value.groups.map((g) => g.id)).size !== value.groups.length ||
    !Array.isArray(value.overriddenFields) ||
    value.overriddenFields.length > 5 ||
    value.overriddenFields.some(
      (v) =>
        !["title", "release_date", "genre", "developer", "favorite"].includes(
          v,
        ),
    ) ||
    value.entries.some(
      (e) =>
        !e ||
        !e.id ||
        !e.sourceId ||
        typeof e.sourceName !== "string" ||
        typeof e.sourceType !== "string" ||
        !["not_started", "playing", "completed", "abandoned"].includes(
          e.completionStatus,
        ),
    ) ||
    value.installations.some(
      (i) =>
        !i ||
        !i.id ||
        !value.entries.some((e) => e.id === i.entryId) ||
        typeof i.rootPathRef !== "string" ||
        typeof i.platform !== "string" ||
        (i.version !== null && typeof i.version !== "string") ||
        !["present", "missing", "unknown"].includes(i.detectionState) ||
        !["legacy", "user"].includes(i.stateOrigin) ||
        (i.groupId !== null && !value.groups.some((g) => g.id === i.groupId)),
    )
  )
    throw new LibraryError(
      "INVALID_RESPONSE",
      "Unexpected editor response. Close and retry.",
    );
  return value;
}
export function createManual(
  metadata: Metadata,
  sourceName: string,
  installation: InstallationInput | null,
) {
  validateMetadata(metadata);
  validateSource(sourceName);
  if (installation) validateInstallation(installation);
  return call<string>("manual_create", {
    request: { metadata, sourceName, installation },
  });
}
export function mutateManual(details: Details, action: Action) {
  if (action.kind === "saveGroup") validateGroupName(action.displayName);
  if (action.kind === "addEntry") validateSource(action.sourceName);
  if (action.kind === "metadata") validateMetadata(action.metadata);
  if (action.kind === "saveInstallation")
    validateInstallation(action.installation);
  return call<null>("manual_mutate", {
    request: { gameId: details.id, expectedRevision: details.revision, action },
  });
}

export function validateGroupName(value: string) {
  if (
    !value.trim() ||
    Array.from(value.trim()).length > 80 ||
    /\p{Cc}/u.test(value)
  )
    throw new LibraryError(
      "INVALID_REQUEST",
      "Enter a group name of 1–80 characters without control characters.",
    );
}

function validateSource(value: string) {
  if (!value.trim() || Array.from(value).length > 80 || /\p{Cc}/u.test(value))
    throw new LibraryError(
      "INVALID_REQUEST",
      "Enter a manual source name of 1–80 characters without control characters.",
    );
}
