-- SPDX-License-Identifier: GPL-3.0-or-later
-- Prototype projection only; provider provenance and launch/mod entities come later.
CREATE TABLE database_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    purpose TEXT NOT NULL CHECK (purpose IN ('application', 'benchmark')),
    generator_version INTEGER,
    dataset_seed TEXT,
    dataset_games INTEGER,
    generation_complete INTEGER NOT NULL DEFAULT 0 CHECK (generation_complete IN (0, 1))
) STRICT;
INSERT INTO database_metadata (singleton, purpose) VALUES (1, 'application');

CREATE TABLE games (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    sort_title TEXT NOT NULL CHECK (length(sort_title) > 0),
    release_date TEXT,
    genre TEXT,
    developer TEXT,
    favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX ix_games_sort ON games(sort_title, id);
CREATE INDEX ix_games_release ON games(release_date, id);

CREATE TABLE library_sources (
    id TEXT PRIMARY KEY NOT NULL,
    source_type TEXT NOT NULL,
    display_name TEXT NOT NULL
) STRICT;

CREATE TABLE library_entries (
    id TEXT PRIMARY KEY NOT NULL,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE RESTRICT,
    source_id TEXT NOT NULL REFERENCES library_sources(id) ON DELETE RESTRICT,
    external_item_id TEXT,
    completion_status TEXT NOT NULL DEFAULT 'not_started'
        CHECK (completion_status IN ('not_started', 'playing', 'completed', 'abandoned')),
    added_at_ms INTEGER NOT NULL,
    UNIQUE(source_id, external_item_id)
) STRICT;
CREATE INDEX ix_library_game ON library_entries(game_id);

CREATE TABLE game_installations (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL REFERENCES library_entries(id) ON DELETE RESTRICT,
    root_path_ref TEXT NOT NULL,
    platform TEXT NOT NULL,
    version TEXT,
    detection_state TEXT NOT NULL CHECK (detection_state IN ('present', 'missing', 'unknown')),
    UNIQUE(entry_id, root_path_ref)
) STRICT;
CREATE INDEX ix_installations_entry ON game_installations(entry_id, detection_state);
