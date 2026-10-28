-- SPDX-License-Identifier: GPL-3.0-or-later
-- Existing projections remain unchanged. Legacy data is never inferred to be user-owned.
ALTER TABLE games ADD COLUMN origin TEXT NOT NULL DEFAULT 'legacy'
    CHECK (origin IN ('legacy', 'manual'));
ALTER TABLE games ADD COLUMN revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0);
ALTER TABLE game_installations ADD COLUMN state_origin TEXT NOT NULL DEFAULT 'legacy'
    CHECK (state_origin IN ('legacy', 'user'));
-- A row with NULL value is an explicit user choice of unknown, not no override.
CREATE TABLE game_field_overrides (
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE RESTRICT,
    field TEXT NOT NULL CHECK (field IN ('title', 'release_date', 'genre', 'developer', 'favorite')),
    value TEXT,
    PRIMARY KEY (game_id, field)
) STRICT;
