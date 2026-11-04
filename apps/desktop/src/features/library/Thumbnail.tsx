// SPDX-License-Identifier: GPL-3.0-or-later
import { memo, useEffect, useState } from "react";
import { artworkQueue, type ArtworkResult } from "./artwork";
export type ThumbnailStatus = "loading" | "ready" | "missing" | "failed";
export const Thumbnail = memo(function Thumbnail({
  id,
  visible,
  revision = 0,
  onStatusChange,
}: {
  id?: string;
  visible: boolean;
  revision?: number;
  onStatusChange?: (status: ThumbnailStatus) => void;
}) {
  const [loaded, setLoaded] = useState<{ id: string; result: ArtworkResult }>();
  const [broken, setBroken] = useState<string>();
  const [decoded, setDecoded] = useState<string>();
  const identity = `${id}:${revision}`;
  useEffect(() => {
    if (!id) return;
    let current = true;
    const release = artworkQueue.subscribe(id, visible ? 0 : 1, (result) => {
      if (current) setLoaded({ id: identity, result });
    });
    return () => {
      current = false;
      release();
    };
  }, [id, visible, identity]);
  const result = loaded && loaded.id === identity ? loaded.result : undefined;
  const url = broken === identity ? null : result?.dataUrl;
  useEffect(() => {
    if (!onStatusChange) return;
    // Admission/shutdown delays and undecoded images are not confirmed failures.
    const status: ThumbnailStatus =
      broken === identity
        ? "failed"
        : !result ||
            ["loading", "busy", "stopping", "deferred"].includes(result.status)
          ? "loading"
          : result.dataUrl
            ? decoded === identity
              ? "ready"
              : "loading"
            : result.status === "missing"
              ? "missing"
              : "failed";
    onStatusChange(status);
  }, [broken, decoded, identity, result, onStatusChange]);
  return (
    <span
      className="thumbnail"
      data-artwork-status={
        broken === identity ? "failed" : (result?.status ?? "loading")
      }
      data-artwork-ready-at={result && !url ? performance.now() : undefined}
      aria-hidden="true"
    >
      {url ? (
        <img
          src={url}
          alt=""
          width={48}
          height={27}
          decoding="async"
          onLoad={(e) => {
            if (onStatusChange) setDecoded(identity);
            e.currentTarget.parentElement!.dataset.artworkReadyAt = String(
              performance.now(),
            );
          }}
          onError={() => setBroken(identity)}
        />
      ) : (
        <span>◇</span>
      )}
    </span>
  );
});
