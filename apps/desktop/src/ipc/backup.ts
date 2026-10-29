// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";

export interface Snapshot {
  formatVersion: number;
  applicationVersion: string;
  schemaVersion: number;
  bytes: number;
  counts: {
    games: number;
    librarySources: number;
    libraryEntries: number;
    installations: number;
  };
}
export interface Candidate {
  token: number;
  snapshot: Snapshot;
}
export interface BackupStatus {
  available: boolean;
  restartRequired: boolean;
  outcome: "restored" | "recovered" | "pendingRestart" | null;
  candidate: Candidate | null;
}
const integer = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
function object(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}
export function snapshot(v: unknown): Snapshot {
  if (
    !object(v) ||
    v.formatVersion !== 1 ||
    typeof v.applicationVersion !== "string" ||
    v.applicationVersion.length > 64 ||
    !integer(v.schemaVersion) ||
    !integer(v.bytes) ||
    !object(v.counts) ||
    ![
      v.counts.games,
      v.counts.librarySources,
      v.counts.libraryEntries,
      v.counts.installations,
    ].every(integer)
  )
    throw new Error("Invalid backup response. Check the desktop connection.");
  return v as unknown as Snapshot;
}
function candidate(v: unknown): Candidate {
  if (!object(v) || !integer(v.token) || v.token === 0)
    throw new Error("Invalid restore candidate response.");
  return { token: v.token, snapshot: snapshot(v.snapshot) };
}
export async function backupStatus(): Promise<BackupStatus> {
  const v: unknown = await invoke("backup_get_status");
  if (
    !object(v) ||
    typeof v.available !== "boolean" ||
    typeof v.restartRequired !== "boolean" ||
    ![null, "restored", "recovered", "pendingRestart"].includes(
      v.outcome as null,
    )
  )
    throw new Error("Invalid maintenance status response.");
  return {
    available: v.available,
    restartRequired: v.restartRequired,
    outcome: v.outcome as BackupStatus["outcome"],
    candidate: v.candidate === null ? null : candidate(v.candidate),
  };
}
export async function createBackup(): Promise<Snapshot | null> {
  const v: unknown = await invoke("backup_create");
  return v === null ? null : snapshot(v);
}
export async function selectRestore(): Promise<Candidate | null> {
  const v: unknown = await invoke("backup_select_restore");
  return v === null ? null : candidate(v);
}
export const cancelBackup = () => invoke<void>("backup_cancel");
export const cancelRestore = () => invoke<void>("backup_cancel_restore");
export const confirmRestore = (token: number) =>
  invoke<void>("backup_confirm_restore", { token });
export const restartForRestore = () => invoke<void>("backup_restart");
export function backupError(v: unknown): string {
  if (object(v) && typeof v.message === "string") return v.message;
  return v instanceof Error
    ? v.message
    : "Library maintenance failed. Existing library and recovery files were preserved.";
}
