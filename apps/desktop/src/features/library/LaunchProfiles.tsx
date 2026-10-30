// SPDX-License-Identifier: GPL-3.0-or-later
import { useRef, useState } from "react";
import type { Action, Details } from "../../ipc/manual";
import {
  type Configuration,
  type PathRef,
  type Profile,
  type ProfileAction,
  validateConfiguration,
} from "../../ipc/launchProfiles";

const empty: Configuration = {
  displayName: "",
  launcherKind: "standalone",
  executableRef: { kind: "installationRelative", value: "" },
  arguments: [],
  workingDirectoryRef: null,
};
export function LaunchProfiles({
  details,
  save,
  confirm,
}: {
  details: Details;
  save: (action: Action) => Promise<boolean>;
  confirm: (action: Action, message: string) => void;
}) {
  const [editing, setEditing] = useState<Profile | null>(null);
  const [installationId, setInstallationId] = useState("");
  const [config, setConfig] = useState<Configuration>(empty);
  const [argumentsText, setArgumentsText] = useState("[]");
  const [error, setError] = useState("");
  const name = useRef<HTMLInputElement>(null);
  const action = (a: ProfileAction): Action => ({
    kind: "launchProfile",
    action: a,
  });
  function reset() {
    setEditing(null);
    setConfig(empty);
    setArgumentsText("[]");
    setError("");
  }
  function edit(p: Profile) {
    setEditing(p);
    setInstallationId(p.installationId);
    setConfig(p.configuration);
    setArgumentsText(JSON.stringify(p.configuration.arguments));
    setError("");
    name.current?.focus();
  }
  function reference(
    label: string,
    value: PathRef,
    change: (p: PathRef) => void,
  ) {
    return (
      <>
        <label>
          {label} kind
          <select
            value={value.kind}
            onChange={(e) =>
              change({ ...value, kind: e.target.value as PathRef["kind"] })
            }
          >
            <option value="installationRelative">Installation relative</option>
            <option value="absolutePath">Absolute Windows path</option>
          </select>
        </label>
        <label>
          {label}
          <input
            required
            value={value.value}
            maxLength={2048}
            onChange={(e) => change({ ...value, value: e.target.value })}
          />
        </label>
      </>
    );
  }
  return (
    <details className="launch-profiles">
      <summary>LaunchProfiles ({details.profiles.length})</summary>
      <p>
        Configuration only. Use the separate Launch action to execute.
        References receive lexical validation only; no paths are opened or
        checked. Up to 100 profiles per game.
      </p>
      {details.installations.map((i) => (
        <section key={i.id} aria-label={`Profiles for ${i.id}`}>
          <h4>
            {i.rootPathRef} · {i.id}
          </h4>
          <ul>
            {details.profiles
              .filter((p) => p.installationId === i.id)
              .map((p) => (
                <li key={p.id}>
                  <span>
                    {p.configuration.displayName} · {p.id.slice(0, 8)} ·
                    revision {p.revision}
                    {details.defaultProfileId === p.id ? " · Default" : ""}
                  </span>
                  <button type="button" onClick={() => edit(p)}>
                    Edit profile {p.id.slice(0, 8)}
                  </button>
                  <button
                    type="button"
                    disabled={details.profiles.length >= 100}
                    onClick={() =>
                      void save(
                        action({
                          operation: "duplicate",
                          profileId: p.id,
                          expectedProfileRevision: p.revision,
                        }),
                      )
                    }
                  >
                    Duplicate profile {p.id.slice(0, 8)}
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      confirm(
                        action({
                          operation: "remove",
                          profileId: p.id,
                          expectedProfileRevision: p.revision,
                        }),
                        `Remove LaunchProfile ${p.configuration.displayName} · ${p.id.slice(0, 8)}? Its configuration is deleted. If default, that selection is cleared without replacement. Files remain untouched.`,
                      )
                    }
                  >
                    Remove profile {p.id.slice(0, 8)}
                  </button>
                </li>
              ))}
          </ul>
        </section>
      ))}
      {!details.installations.length && (
        <p>Record installation metadata first to create a profile.</p>
      )}
      <label>
        Default LaunchProfile
        <select
          value={details.defaultProfileId ?? ""}
          onChange={(e) => {
            const p = details.profiles.find((p) => p.id === e.target.value);
            void save(
              action({
                operation: "setDefault",
                profileId: p?.id ?? null,
                expectedProfileRevision: p?.revision ?? null,
              }),
            );
          }}
        >
          <option value="">No default</option>
          {details.profiles.map((p) => (
            <option key={p.id} value={p.id}>
              {p.configuration.displayName} · {p.id.slice(0, 8)} ·{" "}
              {p.installationId.slice(0, 8)}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        disabled={!details.defaultProfileId}
        onClick={() =>
          void save(
            action({
              operation: "setDefault",
              profileId: null,
              expectedProfileRevision: null,
            }),
          )
        }
      >
        Clear default profile
      </button>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          setError("");
          try {
            const parsed: unknown = JSON.parse(argumentsText);
            if (
              !Array.isArray(parsed) ||
              parsed.some((a) => typeof a !== "string")
            )
              throw new Error(
                "Arguments must be a JSON array of literal strings.",
              );
            const configuration = { ...config, arguments: parsed as string[] };
            validateConfiguration(configuration);
            void save(
              action(
                editing
                  ? {
                      operation: "save",
                      profileId: editing.id,
                      expectedProfileRevision: editing.revision,
                      configuration,
                    }
                  : { operation: "create", installationId, configuration },
              ),
            ).then((ok) => {
              if (ok) reset();
            });
          } catch (e) {
            setError((e as Error).message);
          }
        }}
      >
        <h4>
          {editing
            ? `Edit profile ${editing.id.slice(0, 8)}`
            : "New LaunchProfile"}
        </h4>
        {error && <p role="alert">{error}</p>}
        <label>
          Profile installation
          <select
            required
            disabled={!!editing}
            value={installationId}
            onChange={(e) => setInstallationId(e.target.value)}
          >
            <option value="">Choose installation metadata</option>
            {details.installations.map((i) => (
              <option key={i.id} value={i.id}>
                {i.rootPathRef} · {i.id}
              </option>
            ))}
          </select>
        </label>
        <label>
          Profile name
          <input
            ref={name}
            required
            maxLength={160}
            value={config.displayName}
            onChange={(e) =>
              setConfig({ ...config, displayName: e.target.value })
            }
          />
        </label>
        <p>Launcher kind: standalone</p>
        {reference(
          "Executable reference",
          config.executableRef,
          (executableRef) => setConfig({ ...config, executableRef }),
        )}
        <label>
          Literal arguments (JSON array)
          <textarea
            value={argumentsText}
            maxLength={131072}
            onChange={(e) => setArgumentsText(e.target.value)}
            spellCheck={false}
          />
        </label>
        <p>
          Example: ["--name", "two words", ""]. Up to 128 strings; 4096 UTF-16
          units each and 16384 total. Empty arguments are preserved.
        </p>
        <label>
          <input
            type="checkbox"
            checked={config.workingDirectoryRef !== null}
            onChange={(e) =>
              setConfig({
                ...config,
                workingDirectoryRef: e.target.checked
                  ? { kind: "installationRelative", value: "" }
                  : null,
              })
            }
          />
          Supply working-directory reference
        </label>
        {config.workingDirectoryRef &&
          reference(
            "Working-directory reference",
            config.workingDirectoryRef,
            (workingDirectoryRef) =>
              setConfig({ ...config, workingDirectoryRef }),
          )}
        <button
          type="submit"
          disabled={!editing && details.profiles.length >= 100}
        >
          {editing ? "Save profile configuration" : "Create profile"}
        </button>
        <button type="button" onClick={reset}>
          Cancel profile edit
        </button>
      </form>
    </details>
  );
}
