// SPDX-License-Identifier: GPL-3.0-or-later
// Serialized into the benchmark WebView. No production bridge or polling timer.
export function observeArrival(
  index,
  queryKey,
  action,
  timeoutMs = 10000,
  coarseControl = false,
) {
  return new Promise((resolve, reject) => {
    const start = performance.now();
    let observer, timer, frame, coarseTimer, coarseMs, arrivalResult;
    let matched = false;
    const clean = () => {
      observer.disconnect();
      clearTimeout(timer);
      cancelAnimationFrame(frame);
      clearTimeout(coarseTimer);
      document.removeEventListener("scroll", check, true);
    };
    const check = () => {
      if (matched) return;
      const panel = document.querySelector(".library-panel");
      if (
        panel?.dataset.queryReady !== "true" ||
        (queryKey && panel.dataset.queryKey !== queryKey)
      )
        return;
      const row = document.querySelector(
        `.game-row[aria-rowindex="${index + 2}"][aria-busy="false"]`,
      );
      const scroll = document.querySelector(".library-scroll");
      if (!row || !scroll) return;
      const r = row.getBoundingClientRect(),
        box = scroll.getBoundingClientRect();
      if (r.bottom <= box.top || r.top >= box.bottom) return;
      const domMs = performance.now() - start;
      matched = true;
      const id = row.id;
      observer.disconnect();
      document.removeEventListener("scroll", check, true);
      frame = requestAnimationFrame(() => {
        const nextFrameMs = performance.now() - start;
        frame = requestAnimationFrame(() => {
          arrivalResult = {
            startedAt: start,
            index,
            id,
            domMs,
            nextFrameMs,
            secondFrameMs: performance.now() - start,
          };
          if (!coarseControl || coarseMs !== undefined) complete();
        });
      });
    };
    const complete = () => {
      clean();
      resolve({ ...arrivalResult, coarseMs });
    };
    const coarse = () => {
      // Timer quantization control for the same predicate, without CDP trips.
      // This is not a reconstruction of the older transport/polling overhead.
      if (matched) {
        coarseMs = performance.now() - start;
        if (arrivalResult) complete();
        return;
      }
      coarseTimer = setTimeout(coarse, 50);
    };
    observer = new MutationObserver(check);
    observer.observe(document.querySelector(".main-content") ?? document.body, {
      subtree: true,
      childList: true,
      attributes: true,
    });
    document.addEventListener("scroll", check, true);
    timer = setTimeout(() => {
      clean();
      reject(new Error("Row arrival timed out"));
    }, timeoutMs);
    try {
      action();
      check();
      if (coarseControl) coarse();
    } catch (error) {
      clean();
      reject(error);
    }
  });
}

// Supplemental first-target boundary, used only with the IPC timeline. Unlike
// observeArrival it does not wait for all requested pages/overscan to settle.
export function watchTarget(index, queryKey, timeoutMs = 10000) {
  let observer,
    timer,
    resolve,
    reject,
    done = false;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  const clean = () => {
    observer.disconnect();
    clearTimeout(timer);
    document.removeEventListener("scroll", check, true);
  };
  const check = () => {
    const panel = document.querySelector(".library-panel");
    if (panel?.dataset.queryKey !== queryKey || done) return;
    const row = document.querySelector(
      `.game-row[aria-rowindex="${index + 2}"][aria-busy="false"]`,
    );
    const scroll = document.querySelector(".library-scroll");
    if (!row || !scroll) return;
    const r = row.getBoundingClientRect(),
      b = scroll.getBoundingClientRect();
    if (r.bottom <= b.top || r.top >= b.bottom) return;
    done = true;
    clean();
    resolve({ at: performance.now(), id: row.id });
  };
  observer = new MutationObserver(check);
  observer.observe(document.querySelector(".main-content") ?? document.body, {
    subtree: true,
    childList: true,
    attributes: true,
  });
  document.addEventListener("scroll", check, true);
  timer = setTimeout(() => {
    done = true;
    clean();
    reject(new Error("Target arrival timed out"));
  }, timeoutMs);
  check();
  return {
    promise,
    cancel: () => {
      clean();
      if (!done) {
        done = true;
        resolve(null);
      }
    },
  };
}
