// SPDX-License-Identifier: GPL-3.0-or-later
// Tauri invoke cannot cancel a running native command. Keep its permit until it
// settles. Abort removes queued work immediately; it never reaches native IPC.
export class RequestGate {
  private active = false;
  private queue: Array<() => void> = [];
  get queued() {
    return this.queue.length;
  }
  get activeRequests() {
    return Number(this.active);
  }
  async run<T>(signal: AbortSignal, work: () => Promise<T>): Promise<T> {
    await new Promise<void>((resolve, reject) => {
      const start = () => {
        signal.removeEventListener("abort", abort);
        this.active = true;
        resolve();
      };
      const abort = () => {
        this.queue = this.queue.filter((entry) => entry !== start);
        reject(signal.reason);
      };
      if (signal.aborted) {
        reject(signal.reason);
        return;
      }
      if (!this.active) start();
      else {
        this.queue.push(start);
        signal.addEventListener("abort", abort, { once: true });
      }
    });
    try {
      signal.throwIfAborted();
      const result = await work();
      signal.throwIfAborted();
      return result;
    } finally {
      this.active = false;
      this.queue.shift()?.();
    }
  }
}
export const pageGate = new RequestGate();
