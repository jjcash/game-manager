// SPDX-License-Identifier: GPL-3.0-or-later
import { memo, useEffect, useState } from "react";
import { artworkQueue, type ArtworkResult } from "./artwork";
export const Thumbnail = memo(function Thumbnail({
  id,
  visible,
  revision = 0,
}: {
  id?: string;
  visible: boolean;
  revision?: number;
}) {
  const [loaded, setLoaded] = useState<{ id: string; result: ArtworkResult }>();
  const [broken, setBroken] = useState<string>();
  const identity = `${id}:${revision}`;
  useEffect(() => {
    if (!id) return;
    return artworkQueue.subscribe(id, visible ? 0 : 1, (result) =>
      setLoaded({ id: identity, result }),
    );
  }, [id, visible, identity]);
  const result = loaded && loaded.id === identity ? loaded.result : undefined;
  const url = broken === identity ? null : result?.dataUrl;
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
