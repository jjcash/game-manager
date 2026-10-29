// SPDX-License-Identifier: GPL-3.0-or-later
import { LibraryError } from "./library";
export type PathRef = {
  kind: "installationRelative" | "absolutePath";
  value: string;
};
export interface Configuration {
  displayName: string;
  launcherKind: "standalone";
  executableRef: PathRef;
  arguments: string[];
  workingDirectoryRef: PathRef | null;
}
export interface Profile {
  id: string;
  gameId: string;
  installationId: string;
  provenance: "user";
  revision: number;
  configuration: Configuration;
}
export type ProfileAction =
  | {
      operation: "create";
      installationId: string;
      configuration: Configuration;
    }
  | {
      operation: "save";
      profileId: string;
      expectedProfileRevision: number;
      configuration: Configuration;
    }
  | {
      operation: "duplicate" | "remove";
      profileId: string;
      expectedProfileRevision: number;
    }
  | {
      operation: "setDefault";
      profileId: string | null;
      expectedProfileRevision: number | null;
    };

export function validateReference(ref: PathRef) {
  if (
    !ref ||
    typeof ref.value !== "string" ||
    !["installationRelative", "absolutePath"].includes(ref.kind)
  )
    throw new Error("Choose a supported reference kind.");
  const value = ref.value;
  if (!value || Array.from(value).length > 2048 || /\p{Cc}/u.test(value))
    throw new Error("References require 1–2048 characters without controls.");
  let tail = value;
  if (ref.kind === "absolutePath") {
    if (!/^[A-Za-z]:\\/.test(value))
      throw new Error("Use a local absolute Windows drive path.");
    tail = value.slice(3);
    if (!tail) return;
  }
  if (
    tail
      .split("\\")
      .some(
        (c) =>
          !c ||
          c === "." ||
          c === ".." ||
          /[ .]$/.test(c) ||
          /[<>:"/|?*]/.test(c) ||
          /^(CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|COM[1-9¹²³]|LPT[1-9¹²³])$/i.test(
            c.split(".")[0],
          ),
      )
  )
    throw new Error(
      "Use backslash-separated components without traversal, device names, namespaces, reserved punctuation or trailing dots/spaces.",
    );
}
export function validateConfiguration(c: Configuration) {
  try {
    if (
      !c ||
      typeof c.displayName !== "string" ||
      !c.displayName.trim() ||
      Array.from(c.displayName).length > 80 ||
      /\p{Cc}/u.test(c.displayName) ||
      c.launcherKind !== "standalone"
    )
      throw new Error(
        "Enter a profile name of 1–80 characters without controls; only standalone is supported.",
      );
    validateReference(c.executableRef);
    if (c.workingDirectoryRef !== null)
      validateReference(c.workingDirectoryRef);
    if (
      !Array.isArray(c.arguments) ||
      c.arguments.length > 128 ||
      c.arguments.some(
        (a) => typeof a !== "string" || a.length > 4096 || a.includes("\0"),
      ) ||
      c.arguments.reduce((n, a) => n + a.length, 0) > 16384
    )
      throw new Error(
        "Arguments allow 128 literal strings, 4096 UTF-16 units each and 16384 total; NUL is unsupported.",
      );
  } catch (e) {
    throw new LibraryError("INVALID_REQUEST", (e as Error).message);
  }
}
export function validProfile(
  p: Profile,
  gameId: string,
  installations: string[],
) {
  try {
    if (
      !p ||
      typeof p.id !== "string" ||
      !/^[0-9a-f]{32}$/.test(p.id) ||
      p.gameId !== gameId ||
      !installations.includes(p.installationId) ||
      p.provenance !== "user" ||
      !Number.isSafeInteger(p.revision) ||
      p.revision < 0
    )
      return false;
    validateConfiguration(p.configuration);
    return true;
  } catch {
    return false;
  }
}
