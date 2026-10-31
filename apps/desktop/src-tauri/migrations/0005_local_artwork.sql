-- SPDX-License-Identifier: GPL-3.0-or-later
-- Originals are durable files; deleting an assignment never deletes an asset.
CREATE TABLE artwork_assets (
    id TEXT PRIMARY KEY CHECK(length(id)=32 AND id NOT GLOB '*[^0-9a-f]*'),
    format TEXT NOT NULL CHECK(format IN ('png','jpeg')),
    bytes INTEGER NOT NULL CHECK(bytes BETWEEN 1 AND 8388608),
    width INTEGER NOT NULL CHECK(width BETWEEN 1 AND 4096),
    height INTEGER NOT NULL CHECK(height BETWEEN 1 AND 4096),
    fingerprint TEXT NOT NULL CHECK(length(fingerprint)=16 AND fingerprint NOT GLOB '*[^0-9a-f]*'),
    CHECK(width*height<=4194304)
);
CREATE TABLE game_artwork (
    game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES artwork_assets(id) ON DELETE RESTRICT
);
