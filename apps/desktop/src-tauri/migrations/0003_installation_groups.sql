-- SPDX-License-Identifier: GPL-3.0-or-later
-- User organization only: no paths, presence, provider ownership or launch state.
CREATE TABLE installation_groups (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 80)
) STRICT;
CREATE INDEX ix_installation_groups_game ON installation_groups(game_id, id);

CREATE TABLE installation_group_members (
    installation_id TEXT PRIMARY KEY NOT NULL REFERENCES game_installations(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES installation_groups(id) ON DELETE CASCADE
) STRICT;
CREATE INDEX ix_installation_group_members_group ON installation_group_members(group_id);

CREATE TRIGGER installation_group_same_game_insert
BEFORE INSERT ON installation_group_members
WHEN NOT EXISTS (
    SELECT 1 FROM installation_groups g
    JOIN library_entries e ON e.game_id = g.game_id
    JOIN game_installations i ON i.entry_id = e.id
    WHERE g.id = NEW.group_id AND i.id = NEW.installation_id
)
BEGIN SELECT RAISE(ABORT, 'Installation group must belong to the same game'); END;

CREATE TRIGGER installation_group_same_game_update
BEFORE UPDATE ON installation_group_members
WHEN NOT EXISTS (
    SELECT 1 FROM installation_groups g
    JOIN library_entries e ON e.game_id = g.game_id
    JOIN game_installations i ON i.entry_id = e.id
    WHERE g.id = NEW.group_id AND i.id = NEW.installation_id
)
BEGIN SELECT RAISE(ABORT, 'Installation group must belong to the same game'); END;

CREATE TRIGGER installation_group_fixed_game
BEFORE UPDATE OF game_id ON installation_groups
WHEN NEW.game_id <> OLD.game_id
BEGIN SELECT RAISE(ABORT, 'Installation group game identity is immutable'); END;
