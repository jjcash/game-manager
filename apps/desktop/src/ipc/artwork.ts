// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";
import type { Details } from "./manual";

export async function changeArtwork(details: Details, clear: boolean) {
  if (
    !/^[a-zA-Z0-9-]{1,128}$/.test(details.id) ||
    !Number.isSafeInteger(details.revision) ||
    details.revision < 0
  )
    throw { code: "invalidRequest" };
  const result = await invoke<unknown>(
    clear ? "artwork_clear" : "artwork_choose",
    {
      request: { gameId: details.id, expectedRevision: details.revision },
    },
  );
  if (clear) return true;
  if (typeof result !== "boolean") throw { code: "connection" };
  return result;
}
export function artworkError(error: unknown): string {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? error.code
      : "connection";
  switch (code) {
    case "stale":
      return "Game changed. Artwork was not assigned. Review the refreshed details and try again.";
    case "busy":
      return "Artwork work is busy. Wait for it to finish, then try again.";
    case "stopping":
      return "Library is closing. Restart Game Manager to continue.";
    case "oversized":
      return "Choose a PNG or JPEG within the displayed file and pixel limits. Previous artwork was preserved.";
    case "corrupt":
    case "unsupported":
      return "This file is not a supported readable PNG or JPEG. Previous artwork was preserved.";
    case "notFound":
      return "Game no longer exists. Close and reopen the editor.";
    case "unsafePath":
    case "unsafeObject":
      return "Choose a regular local image file without links or special path syntax. Previous artwork was preserved.";
    case "storage":
    case "database":
    case "unavailable":
      return "Artwork could not be saved. Check the local drive, file permissions and free space. Previous artwork was preserved.";
    default:
      return "Artwork result could not be confirmed. Close and reopen the editor before trying again.";
  }
}
