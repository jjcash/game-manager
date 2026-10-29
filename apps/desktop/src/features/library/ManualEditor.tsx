// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import {
  createManual,
  getManual,
  mutateManual,
  type Action,
  type Details,
  type InstallationInput,
  type Metadata,
} from "../../ipc/manual";
import { pageGate } from "./requestGate";
import { InstallationGroups } from "./InstallationGroups";

const emptyMetadata: Metadata = {
  title: "",
  releaseDate: null,
  genre: null,
  developer: null,
  favorite: false,
};
const emptyInstallation: InstallationInput = {
  rootPathRef: "",
  platform: "windows",
  version: null,
  detectionState: "unknown",
};
export function ManualEditor({
  gameId,
  close,
  changed,
}: {
  gameId: string | null;
  close: () => void;
  changed: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const pending = useRef<AbortController | null>(null);
  const [details, setDetails] = useState<Details | null>(null);
  const [metadata, setMetadata] = useState<Metadata>(emptyMetadata);
  const [source, setSource] = useState("Manual");
  const [installation, setInstallation] =
    useState<InstallationInput>(emptyInstallation);
  const [installationId, setInstallationId] = useState<string | null>(null);
  const [entryId, setEntryId] = useState("");
  const [withInstallation, setWithInstallation] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(!!gameId);
  const [error, setError] = useState("");
  const [confirmation, setConfirmation] = useState<{
    action: Action;
    message: string;
  } | null>(null);
  const title = gameId ? "Manage game" : "Add game";
  useEffect(() => {
    const element = dialog.current;
    const previous = document.activeElement as HTMLElement | null;
    element?.showModal();
    return () => {
      pending.current?.abort();
      element?.close();
      if (previous?.isConnected && !previous.matches(":disabled"))
        previous.focus();
      else
        document
          .querySelector<HTMLElement>(".library-toolbar button:not(:disabled)")
          ?.focus();
    };
  }, []);
  useEffect(() => {
    if (!gameId) return;
    const controller = new AbortController();
    void pageGate
      .run(controller.signal, () => getManual(gameId))
      .then((v) => {
        if (controller.signal.aborted) return;
        setDetails(v);
        setMetadata(v.metadata);
        setEntryId(v.entries.find((e) => e.sourceType === "manual")?.id ?? "");
        setLoading(false);
      })
      .catch((e) => {
        if (!controller.signal.aborted) {
          setError((e as Error).message);
          setLoading(false);
        }
      });
    return () => controller.abort();
  }, [gameId]);
  useEffect(() => {
    if (!confirmation) return;
    const previous = document.activeElement as HTMLElement | null;
    dialog.current
      ?.querySelector<HTMLElement>('[role="alertdialog"] button:last-child')
      ?.focus();
    return () => {
      if (previous?.isConnected && !previous.matches(":disabled"))
        previous.focus();
    };
  }, [confirmation]);
  async function run(action?: Action) {
    if (busy || pending.current) return false;
    const controller = new AbortController();
    pending.current = controller;
    setBusy(true);
    setError("");
    try {
      if (!gameId) {
        await pageGate.run(controller.signal, () =>
          createManual(
            metadata,
            source,
            withInstallation ? installation : null,
          ),
        );
        if (controller.signal.aborted) return false;
        changed();
        close();
      } else if (details && action) {
        await pageGate.run(controller.signal, () =>
          mutateManual(details, action),
        );
        if (controller.signal.aborted) return false;
        changed();
        if (action.kind === "removeGame") {
          close();
          return true;
        }
        // Refresh only this bounded editor, preserving drafts on other forms.
        const next = await pageGate.run(controller.signal, () =>
          getManual(gameId),
        );
        if (controller.signal.aborted) return false;
        setDetails(next);
        if (!next.entries.some((e) => e.id === entryId))
          setEntryId(
            next.entries.find((e) => e.sourceType === "manual")?.id ?? "",
          );
        if (
          action.kind === "saveInstallation" ||
          action.kind === "removeInstallation"
        ) {
          setInstallation(emptyInstallation);
          setInstallationId(null);
        }
        setConfirmation(null);
      }
      return true;
    } catch (e) {
      if (!controller.signal.aborted) setError((e as Error).message);
      return false;
    } finally {
      if (pending.current === controller) pending.current = null;
      if (!controller.signal.aborted) setBusy(false);
    }
  }
  const input = (
    label: string,
    field: "title" | "releaseDate" | "genre" | "developer",
    maxLength = 200,
  ) => (
    <label>
      {label}
      <input
        required={field === "title"}
        maxLength={maxLength}
        value={metadata[field] ?? ""}
        onChange={(e) =>
          setMetadata({
            ...metadata,
            [field]:
              field === "title" ? e.target.value : e.target.value || null,
          })
        }
      />
    </label>
  );
  const installationFields = (
    <>
      <label>
        Installation location (metadata only)
        <input
          required
          maxLength={2048}
          value={installation.rootPathRef}
          onChange={(e) =>
            setInstallation({ ...installation, rootPathRef: e.target.value })
          }
        />
      </label>
      <label>
        Platform
        <input
          required
          maxLength={80}
          value={installation.platform}
          onChange={(e) =>
            setInstallation({ ...installation, platform: e.target.value })
          }
        />
      </label>
      <label>
        Version
        <input
          maxLength={200}
          value={installation.version ?? ""}
          onChange={(e) =>
            setInstallation({
              ...installation,
              version: e.target.value || null,
            })
          }
        />
      </label>
      <label>
        Recorded presence
        <select
          value={installation.detectionState}
          onChange={(e) =>
            setInstallation({
              ...installation,
              detectionState: e.target
                .value as InstallationInput["detectionState"],
            })
          }
        >
          <option value="unknown">Unknown</option>
          <option value="present">Present (user asserted)</option>
          <option value="missing">Missing (user asserted)</option>
        </select>
      </label>
    </>
  );
  return (
    <dialog
      ref={dialog}
      className="manual-editor"
      aria-label={title}
      onKeyDown={(event) => {
        if (
          event.key !== "Tab" ||
          event.altKey ||
          event.ctrlKey ||
          event.metaKey
        )
          return;
        // WebView's native dialog traversal can visit the host after the last
        // control. Keep the boundary explicit, including disabled fieldsets.
        const scope =
          event.currentTarget.querySelector('[role="alertdialog"]') ??
          event.currentTarget;
        const controls = scope.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled):not([type="hidden"]), select:not(:disabled), summary',
        );
        const first = controls[0];
        const last = controls[controls.length - 1];
        if (!first) event.preventDefault();
        else if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }}
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) close();
      }}
    >
      <h2>{title}</h2>
      <p>
        Library records only. Locations are user assertions; Game Manager does
        not inspect, install, launch or remove game files.
      </p>
      {error && <p role="alert">{error}</p>}
      {loading && <p>Loading game…</p>}
      <fieldset disabled={busy || loading || (!!gameId && !details)}>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void run(gameId ? { kind: "metadata", metadata } : undefined);
          }}
        >
          <h3>Game metadata</h3>
          {input("Title", "title")}
          {input("Release date (YYYY-MM-DD)", "releaseDate", 10)}
          {input("Genre", "genre")}
          {input("Developer", "developer")}
          <label>
            <input
              type="checkbox"
              checked={metadata.favorite}
              onChange={(e) =>
                setMetadata({ ...metadata, favorite: e.target.checked })
              }
            />
            Favorite
          </label>
          <p>
            Saved fields become explicit user overrides, including cleared
            values.
          </p>
          {!gameId && (
            <>
              <label>
                Manual source name
                <input
                  required
                  maxLength={80}
                  value={source}
                  onChange={(e) => setSource(e.target.value)}
                />
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={withInstallation}
                  onChange={(e) => setWithInstallation(e.target.checked)}
                />
                Record an installation
              </label>
              {withInstallation && installationFields}
            </>
          )}
          <button type="submit">
            {gameId ? "Save metadata" : "Create game"}
          </button>
        </form>
        {details && (
          <>
            <h3>Memberships</h3>
            <ul>
              {details.entries.map((e) => (
                <li key={e.id}>
                  <strong>{e.sourceName}</strong> ({e.sourceType})
                  {e.sourceType === "manual" ? (
                    <>
                      <form
                        onSubmit={(event) => {
                          event.preventDefault();
                          const data = new FormData(event.currentTarget);
                          void run({
                            kind: "updateEntry",
                            entryId: e.id,
                            completionStatus: String(data.get("completion")),
                          });
                        }}
                      >
                        <label>
                          Completion for {e.sourceName}
                          <select
                            name="completion"
                            defaultValue={e.completionStatus}
                          >
                            {[
                              "not_started",
                              "playing",
                              "completed",
                              "abandoned",
                            ].map((s) => (
                              <option key={s} value={s}>
                                {s.replace(/_/g, " ")}
                              </option>
                            ))}
                          </select>
                        </label>
                        <button>Save membership</button>
                      </form>
                      <button
                        disabled={details.entries.length <= 1}
                        onClick={() =>
                          setConfirmation({
                            action: { kind: "removeEntry", entryId: e.id },
                            message: `Remove membership ${e.sourceName} and its ${details.installations.filter((i) => i.entryId === e.id).length} installation metadata records? The canonical game, other memberships and groups stay. Assignments for removed records are removed.`,
                          })
                        }
                      >
                        Remove membership
                      </button>
                    </>
                  ) : (
                    <span>Provider membership is read-only</span>
                  )}
                </li>
              ))}
            </ul>
            <p>
              The last membership is kept. Use Remove manual game to delete the
              canonical record.
            </p>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void run({ kind: "addEntry", sourceName: source });
              }}
            >
              <label>
                Manual source name
                <input
                  required
                  maxLength={80}
                  value={source}
                  onChange={(e) => setSource(e.target.value)}
                />
              </label>
              <button>Add membership</button>
            </form>
            <h3>Installation metadata</h3>
            <InstallationGroups
              details={details}
              save={run}
              confirm={(action, message) =>
                setConfirmation({ action, message })
              }
            />
            <ul>
              {details.installations.map((i) => (
                <li key={i.id}>
                  {i.rootPathRef} · {i.detectionState} · {i.stateOrigin}
                  <span>
                    {" "}
                    ·{" "}
                    {i.groupId
                      ? `${details.groups.find((g) => g.id === i.groupId)?.displayName} · ${i.groupId.slice(0, 8)}`
                      : "Ungrouped"}
                  </span>
                  {details.entries.find((e) => e.id === i.entryId)
                    ?.sourceType === "manual" && (
                    <>
                      <button
                        onClick={() => {
                          setInstallation(i);
                          setInstallationId(i.id);
                          setEntryId(i.entryId);
                        }}
                      >
                        Edit installation
                      </button>
                      <button
                        onClick={() =>
                          setConfirmation({
                            action: {
                              kind: "removeInstallation",
                              installationId: i.id,
                            },
                            message:
                              "Remove this installation metadata record? Files remain untouched.",
                          })
                        }
                      >
                        Remove installation record
                      </button>
                    </>
                  )}
                </li>
              ))}
            </ul>
            {!details.installations.length && (
              <p>
                No installation recorded. This game can be owned without being
                installed.
              </p>
            )}
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void run({
                  kind: "saveInstallation",
                  entryId,
                  installationId,
                  installation,
                });
              }}
            >
              <label>
                Installation membership
                <select
                  required
                  value={entryId}
                  onChange={(e) => {
                    setEntryId(e.target.value);
                    setInstallationId(null);
                  }}
                >
                  <option value="">Choose a manual membership</option>
                  {details.entries
                    .filter((e) => e.sourceType === "manual")
                    .map((e) => (
                      <option key={e.id} value={e.id}>
                        {e.sourceName} · {e.id.slice(0, 8)}
                      </option>
                    ))}
                </select>
              </label>
              {installationFields}
              <button>
                {installationId
                  ? "Save installation record"
                  : "Add installation record"}
              </button>
              {installationId && (
                <button
                  type="button"
                  onClick={() => {
                    setInstallationId(null);
                    setInstallation(emptyInstallation);
                  }}
                >
                  Cancel installation edit
                </button>
              )}
            </form>
            {details.origin === "manual" &&
              details.entries.every((e) => e.sourceType === "manual") && (
                <button
                  onClick={() =>
                    setConfirmation({
                      action: { kind: "removeGame" },
                      message: `Remove this canonical game, all ${details.entries.length} manual memberships, all ${details.installations.length} installation metadata records, all ${details.groups.length} installation groups and its user overrides? Game files remain untouched.`,
                    })
                  }
                >
                  Remove manual game
                </button>
              )}
          </>
        )}
        {confirmation && (
          <div role="alertdialog" aria-label="Confirm removal">
            <p>{confirmation.message}</p>
            <button onClick={() => void run(confirmation.action)}>
              Confirm removal
            </button>
            <button onClick={() => setConfirmation(null)}>Keep records</button>
          </div>
        )}
      </fieldset>
      <button disabled={busy} onClick={close}>
        Close editor
      </button>
    </dialog>
  );
}
