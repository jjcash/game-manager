// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it } from "vitest";
import {
  type Configuration,
  validateConfiguration,
  validateReference,
  validProfile,
} from "./launchProfiles";
const config: Configuration = {
  displayName: "日本語 🎮",
  launcherKind: "standalone",
  executableRef: { kind: "installationRelative", value: "bin\\ゲーム.exe" },
  arguments: ["two words", '"quoted"', "", "%TEMP% & |", "🎮"],
  workingDirectoryRef: null,
};
it("preserves typed Windows references and literal argument boundaries", () => {
  expect(() => validateConfiguration(config)).not.toThrow();
  for (const value of ["C:\\", "Z:\\日本語\\game.exe"])
    expect(() =>
      validateReference({ kind: "absolutePath", value }),
    ).not.toThrow();
  for (const value of [
    "..\\escape",
    "C:game",
    "C:\\game",
    "\\\\host\\x",
    "\\\\?\\C:\\x",
    "a/b",
    "a\\..\\b",
    "NUL.exe",
    "LPT²",
    "a.",
    "a ",
    "x:stream",
    "a\\\\b",
    "",
  ])
    expect(() =>
      validateReference({ kind: "installationRelative", value }),
    ).toThrow();
  for (const value of ["C:game", "\\root", "\\\\host\\x", "C:\\..\\x"])
    expect(() => validateReference({ kind: "absolutePath", value })).toThrow();
  for (const args of [
    Array(129).fill(""),
    ["x".repeat(4097)],
    Array(5).fill("x".repeat(4096)),
    ["\0"],
  ])
    expect(() =>
      validateConfiguration({ ...config, arguments: args }),
    ).toThrow();
  expect(() =>
    validateConfiguration({ ...config, arguments: Array(128).fill("") }),
  ).not.toThrow();
  expect(() =>
    validateConfiguration({ ...config, displayName: "🎮".repeat(81) }),
  ).toThrow();
  expect(() =>
    validateConfiguration({
      ...config,
      workingDirectoryRef: { kind: "absolutePath", value: "C:\\synthetic" },
    }),
  ).not.toThrow();
});
it("rejects malformed profile associations and revisions", () => {
  const p = {
    id: "a".repeat(32),
    gameId: "g",
    installationId: "i",
    provenance: "user" as const,
    revision: 7,
    configuration: config,
  };
  expect(validProfile(p, "g", ["i"])).toBe(true);
  for (const value of [
    { ...p, gameId: "other" },
    { ...p, installationId: "other" },
    { ...p, revision: -1 },
    { ...p, configuration: { ...config, arguments: [9] } },
  ])
    expect(validProfile(value as typeof p, "g", ["i"])).toBe(false);
});
