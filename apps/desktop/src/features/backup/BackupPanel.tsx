// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import {
  backupError,
  backupStatus,
  cancelBackup,
  cancelRestore,
  confirmRestore,
  createBackup,
  restartForRestore,
  selectRestore,
  type BackupStatus,
  type Candidate,
} from "../../ipc/backup";

function Confirmation({
  candidate,
  busy,
  confirm,
  cancel,
}: {
  candidate: Candidate;
  busy: boolean;
  confirm: () => void;
  cancel: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const element = dialog.current;
    element?.showModal();
    return () => {
      element?.close();
      if (previous?.isConnected && !previous.matches(":disabled"))
        previous.focus();
      else
        document
          .querySelector<HTMLButtonElement>(
            ".backup-actions button:not(:disabled)",
          )
          ?.focus();
    };
  }, []);
  return (
    <dialog
      ref={dialog}
      className="manual-editor"
      aria-labelledby="restore-title"
      aria-describedby="restore-consequences"
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) cancel();
      }}
      onKeyDown={(e) => {
        if (e.key !== "Tab") return;
        const controls = Array.from(
          e.currentTarget.querySelectorAll<HTMLButtonElement>(
            "button:not(:disabled)",
          ),
        );
        const first = controls[0],
          last = controls[controls.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }}
    >
      <h2 id="restore-title">Replace current library?</h2>
      <p id="restore-consequences">
        This replaces your entire library with the validated backup (
        {candidate.snapshot.counts.games.toLocaleString()} games). Records are
        not merged. Installation groups are replaced too. Unsaved edits will be
        lost. Game files are never changed.
      </p>
      <p className="muted mt-3">
        A verified copy of your current library will be preserved. The library
        closes after confirmation; restart is required to apply the backup. If
        replacement fails, Game Manager attempts to recover the preserved
        library before opening it. Recovery files are kept for inspection.
      </p>
      <div className="mt-4">
        <button type="button" autoFocus disabled={busy} onClick={cancel}>
          Cancel restore
        </button>
        <button type="button" disabled={busy} onClick={confirm}>
          Confirm library replacement
        </button>
      </div>
      {busy && (
        <p role="status">
          Closing library connections and preserving your current library…
        </p>
      )}
    </dialog>
  );
}
export function BackupPanel() {
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [busy, setBusy] = useState<
    "backup" | "stage" | "confirm" | "restart" | "cancel" | null
  >(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [confirmation, setConfirmation] = useState(false);
  useEffect(() => {
    let active = true;
    void backupStatus()
      .then((v) => {
        if (active) setStatus(v);
      })
      .catch((e) => {
        if (active) setError(backupError(e));
      });
    return () => {
      active = false;
    };
  }, []);
  async function run(kind: NonNullable<typeof busy>) {
    if (busy) return;
    setBusy(kind);
    setError("");
    setMessage("");
    try {
      if (kind === "backup") {
        const result = await createBackup();
        setMessage(
          result
            ? `Backup created and verified · ${result.counts.games.toLocaleString()} games · ${(result.bytes / 1024 / 1024).toFixed(2)} MiB. Store this unencrypted file somewhere private.`
            : "No backup created. Your library is unchanged.",
        );
      }
      if (kind === "stage") {
        const result = await selectRestore();
        setMessage(
          result
            ? "Backup validated. Review its contents before confirming replacement."
            : "Restore selection cancelled. Your library is unchanged.",
        );
      }
      if (kind === "cancel") {
        await cancelRestore();
        setConfirmation(false);
        setMessage("Restore cancelled. Your library is unchanged.");
      }
      if (kind === "confirm" && status?.candidate) {
        await confirmRestore(status.candidate.token);
        setConfirmation(false);
        setMessage(
          "Recovery copy preserved. Restart to apply and verify the replacement.",
        );
      }
      if (kind === "restart") {
        setMessage(
          "Restarting to apply and verify the library. Recovery files will be retained.",
        );
        await restartForRestore();
      }
    } catch (e) {
      setError(backupError(e));
      setConfirmation(false);
    } finally {
      try {
        setStatus(await backupStatus());
      } catch (e) {
        setError(backupError(e));
      }
      setBusy(null);
    }
  }
  const candidate = status?.candidate;
  return (
    <section
      className="backup-panel placeholder-panel"
      aria-labelledby="backup-title"
      aria-busy={busy !== null}
    >
      <div className="panel-heading">
        <h2 id="backup-title">Offline library backup</h2>
      </div>
      <div className="backup-content">
        <p>
          Create a local SQLite snapshot of your library, sources, installation
          metadata and saved overrides. Artwork, game files and mod files are
          excluded.
        </p>
        <p className="muted mt-3">
          Choose a new filename. Existing files are never overwritten. Backups
          are unencrypted and can contain private library information and
          recorded paths.
        </p>
        {status && !status.available && (
          <p role="status">
            Backup and restore are unavailable for benchmark or synthetic job
            databases.
          </p>
        )}
        {status?.outcome === "restored" && (
          <p role="status">
            Restore completed and verified after restart. The prior library and
            recovery journal remain preserved.
          </p>
        )}
        {status?.outcome === "recovered" && (
          <p role="status">
            The restore did not complete. Your preserved prior library was
            recovered and verified. Candidate, previous library and recovery
            journal remain available in the application database directory.
          </p>
        )}
        {status?.restartRequired && (
          <p role="status">
            The library is closed. Restart Game Manager to finish the confirmed
            operation or reopen the preserved current library after a
            preparation failure.
          </p>
        )}
        <div className="backup-actions mt-4">
          <button
            type="button"
            disabled={
              !status?.available ||
              !!busy ||
              !!candidate ||
              status.restartRequired
            }
            onClick={() => void run("backup")}
          >
            Create backup…
          </button>
          <button
            type="button"
            disabled={
              !status?.available ||
              !!busy ||
              !!candidate ||
              status.restartRequired
            }
            onClick={() => void run("stage")}
          >
            Select backup to restore…
          </button>
          {busy === "backup" && (
            <button
              type="button"
              onClick={() => {
                void cancelBackup()
                  .then(() =>
                    setMessage(
                      "Cancellation requested. Waiting for snapshot work to finish; publication may already have completed.",
                    ),
                  )
                  .catch((e) => setError(backupError(e)));
              }}
            >
              Cancel backup
            </button>
          )}
        </div>
        {busy === "backup" && (
          <p role="status">
            Choose a destination, then wait while the snapshot is created and
            verified…
          </p>
        )}
        {busy === "stage" && (
          <p role="status">
            Choose a backup, then wait while a private candidate is validated…
          </p>
        )}
        {candidate && !status.restartRequired && (
          <div className="backup-candidate mt-4">
            <h3>Validated backup</h3>
            <p>
              {candidate.snapshot.counts.games.toLocaleString()} games ·{" "}
              {candidate.snapshot.counts.librarySources.toLocaleString()}{" "}
              sources ·{" "}
              {candidate.snapshot.counts.libraryEntries.toLocaleString()}{" "}
              memberships ·{" "}
              {candidate.snapshot.counts.installations.toLocaleString()}{" "}
              installations
            </p>
            <p className="muted">
              Schema {candidate.snapshot.schemaVersion} ·{" "}
              {(candidate.snapshot.bytes / 1024 / 1024).toFixed(2)} MiB.
              Restoring replaces your entire current library after restart.
            </p>
            {candidate.snapshot.schemaVersion === 2 && (
              <p>
                This older backup has no installation groups. Its restored
                library upgrades automatically; the selected backup stays
                unchanged.
              </p>
            )}
            <button
              type="button"
              disabled={!!busy}
              onClick={() => setConfirmation(true)}
            >
              Review replacement…
            </button>
            <button
              type="button"
              disabled={!!busy}
              onClick={() => void run("cancel")}
            >
              Discard selected backup
            </button>
          </div>
        )}
        {status?.restartRequired && (
          <button
            type="button"
            disabled={!!busy}
            onClick={() => void run("restart")}
          >
            Restart and finish
          </button>
        )}
        {message && (
          <p role="status" className="mt-3">
            {message}
          </p>
        )}
        {error && (
          <p role="alert" className="mt-3">
            {error}
          </p>
        )}
        {!status && (
          <button
            type="button"
            disabled={!!busy}
            onClick={() => {
              void backupStatus()
                .then((v) => {
                  setStatus(v);
                  setError("");
                })
                .catch((e) => setError(backupError(e)));
            }}
          >
            Retry library status
          </button>
        )}
      </div>
      {confirmation && candidate && (
        <Confirmation
          candidate={candidate}
          busy={busy !== null}
          confirm={() => void run("confirm")}
          cancel={() => void run("cancel")}
        />
      )}
    </section>
  );
}
