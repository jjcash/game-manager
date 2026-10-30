// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";
import type { Details } from "./manual";
import type { Profile } from "./launchProfiles";

export class LaunchError extends Error {
  constructor(
    public readonly code: string,
    message: string,
  ) {
    super(message);
  }
}
const messages: Record<string, string> = {
  staleContext:
    "Configuration changed. Review the refreshed profiles and launch again.",
  missingOrUnrelated:
    "The game or profile is no longer available. Refresh its details.",
  unsupportedContext:
    "Only user-owned standalone profiles on manual Windows installations can launch.",
  missingExecutable: "Executable not found. Check the saved profile and drive.",
  invalidWorkingDirectory: "Working directory is unavailable or invalid.",
  databaseUnavailable:
    "Database unavailable. Wait for startup or restart Game Manager.",
  stopping:
    "Restore or shutdown is in progress. Restart Game Manager to continue.",
  permissionDenied: "Windows denied access. Check file permissions.",
  elevatedParent:
    "Run Game Manager without administrator privileges to launch.",
  busy: "Another launch request is in progress. Try again after it completes.",
  unsupportedExecutable:
    "Select an explicit Windows .exe in the saved profile.",
  outcomeUnknown:
    "Launch outcome could not be reported. Check Windows before deliberately trying again.",
};
export function unsupportedReason(
  details: Details,
  profile: Profile,
): string | null {
  const installation = details.installations.find(
    (i) => i.id === profile.installationId,
  );
  const entry = details.entries.find((e) => e.id === installation?.entryId);
  if (
    profile.provenance !== "user" ||
    profile.configuration.launcherKind !== "standalone" ||
    installation?.platform !== "windows" ||
    entry?.sourceType !== "manual"
  )
    return "Unsupported profile or source";
  if (!/\.exe$/i.test(profile.configuration.executableRef.value))
    return "Requires an explicit .exe";
  return null;
}
export async function launchStandalone(details: Details, profile: Profile) {
  try {
    const result = await invoke<{ pid: number }>("standalone_launch", {
      request: {
        gameId: details.id,
        profileId: profile.id,
        expectedProfileRevision: profile.revision,
        expectedGameRevision: details.revision,
      },
    });
    if (!Number.isInteger(result?.pid) || result.pid <= 0)
      throw new LaunchError("outcomeUnknown", messages.outcomeUnknown);
    return result;
  } catch (e) {
    if (e instanceof LaunchError) throw e;
    const code =
      typeof e === "object" &&
      e !== null &&
      "code" in e &&
      typeof e.code === "string"
        ? e.code
        : "outcomeUnknown";
    // Never render backend strings, paths, argument arrays or environment values.
    throw new LaunchError(
      code,
      messages[code] ??
        "Windows could not create the process. Check the saved configuration and file permissions.",
    );
  }
}
