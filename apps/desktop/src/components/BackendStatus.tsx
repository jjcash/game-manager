import { useEffect, useState } from "react";
import { getAppStatus, type AppStatus } from "../ipc/app";

type Connection =
  | { state: "checking" }
  | { state: "ready"; status: AppStatus }
  | { state: "error"; message: string };

export function BackendStatus() {
  const [connection, setConnection] = useState<Connection>({
    state: "checking",
  });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    getAppStatus().then(
      (status) => {
        if (!cancelled) setConnection({ state: "ready", status });
      },
      (error: unknown) => {
        if (!cancelled) {
          setConnection({
            state: "error",
            message:
              error instanceof Error
                ? error.message
                : "Connection failed. Try again.",
          });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  return (
    <section className="connection-panel" aria-labelledby="connection-title">
      <div className="flex items-start gap-3">
        <span
          className={`connection-dot ${connection.state}`}
          aria-hidden="true"
        />
        <div className="min-w-0">
          <h2 id="connection-title" className="text-sm font-semibold">
            Desktop connection
          </h2>
          <div
            role="status"
            aria-live="polite"
            aria-atomic="true"
            className="connection-message"
          >
            {connection.state === "checking" && "Checking the Rust backend…"}
            {connection.state === "ready" && (
              <>
                Rust backend connected · {connection.status.appName}{" "}
                {connection.status.version}
              </>
            )}
            {connection.state === "error" && connection.message}
          </div>
        </div>
      </div>
      <button
        type="button"
        className="check-button shrink-0"
        disabled={connection.state === "checking"}
        onClick={() => {
          setConnection({ state: "checking" });
          setAttempt((value) => value + 1);
        }}
      >
        {connection.state === "error" ? "Try again" : "Check again"}
      </button>
    </section>
  );
}
