// SPDX-License-Identifier: GPL-3.0-or-later

import { invoke } from "@tauri-apps/api/core";

export interface AppStatus {
  appName: "Game Manager";
  version: string;
  backend: "rust";
}

function isAppStatus(value: unknown): value is AppStatus {
  if (typeof value !== "object" || value === null) return false;
  return (
    "appName" in value &&
    value.appName === "Game Manager" &&
    "version" in value &&
    typeof value.version === "string" &&
    value.version.length > 0 &&
    "backend" in value &&
    value.backend === "rust"
  );
}

export async function getAppStatus(): Promise<AppStatus> {
  let response: unknown;
  try {
    response = await invoke<unknown>("app_get_status");
  } catch {
    throw new Error(
      "Could not reach the desktop backend. Try again in the Game Manager desktop window.",
    );
  }
  if (!isAppStatus(response)) {
    throw new Error(
      "The desktop backend returned an unexpected status. Restart Game Manager.",
    );
  }
  return response;
}
