// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import type { Details } from "../../ipc/manual";
import {
  LaunchError,
  launchStandalone,
  unsupportedReason,
} from "../../ipc/standaloneLaunch";

export function StandaloneLaunch({
  details,
  refresh,
}: {
  details: Details;
  refresh: () => Promise<void>;
}) {
  const [selected, setSelected] = useState(() => {
    const profile = details.profiles.find(
      (p) => p.id === details.defaultProfileId,
    );
    return profile && !unsupportedReason(details, profile) ? profile.id : "";
  });
  const [busy, setBusy] = useState(false);
  const inFlight = useRef(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState(false);
  const profile = details.profiles.find((p) => p.id === selected);
  const reason = profile
    ? unsupportedReason(details, profile)
    : "Select a launch profile explicitly.";
  useEffect(() => {
    if (!success) return;
    const timer = window.setTimeout(() => setSuccess(false), 5000);
    return () => window.clearTimeout(timer);
  }, [success]);
  async function launch() {
    if (inFlight.current) return;
    if (!profile || reason) {
      setError(reason ?? "Select a launch profile.");
      return;
    }
    inFlight.current = true;
    setBusy(true);
    setError("");
    setSuccess(false);
    try {
      await launchStandalone(details, profile);
      setSuccess(true);
    } catch (e) {
      setError((e as Error).message);
      if (
        e instanceof LaunchError &&
        ["staleContext", "missingOrUnrelated"].includes(e.code)
      ) {
        setSelected("");
        try {
          await refresh();
        } catch {
          setError(
            "Could not refresh profiles. Close and reopen game details before launching.",
          );
        }
      }
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <section aria-label="Standalone launch">
      <h3>Launch game</h3>
      <label>
        Launch profile
        <select
          value={selected}
          disabled={busy}
          onChange={(e) => {
            setSelected(e.target.value);
            setError("");
            setSuccess(false);
          }}
        >
          <option value="">Select a profile…</option>
          {details.profiles.map((p) => {
            const why = unsupportedReason(details, p);
            return (
              <option key={p.id} value={p.id} disabled={!!why}>
                {p.configuration.displayName} · {p.id.slice(0, 8)}
                {why ? ` — ${why}` : ""}
              </option>
            );
          })}
        </select>
      </label>
      <p id="standalone-launch-help">
        {busy
          ? "Sending launch request…"
          : (reason ??
            "Runs the saved executable with your ordinary Windows privileges. Selection does not change the saved default.")}
      </p>
      <button
        type="button"
        disabled={busy || !!reason}
        aria-describedby="standalone-launch-help"
        onClick={() => void launch()}
      >
        {busy ? "Launching…" : "Launch"}
      </button>
      {error && <p role="alert">{error}</p>}
      <p role="status">{success ? "Launch request sent to Windows." : ""}</p>
    </section>
  );
}
