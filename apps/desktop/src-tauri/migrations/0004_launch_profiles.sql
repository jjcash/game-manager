-- SPDX-License-Identifier: GPL-3.0-or-later
-- Configuration only. References are never resolved or executed by persistence.
CREATE TABLE launch_profiles (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    installation_id TEXT NOT NULL REFERENCES game_installations(id) ON DELETE RESTRICT,
    provenance TEXT NOT NULL CHECK (provenance = 'user'),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 80),
    launcher_kind TEXT NOT NULL CHECK (launcher_kind = 'standalone'),
    executable_kind TEXT NOT NULL CHECK (executable_kind IN ('installationRelative','absolutePath')),
    executable_value TEXT NOT NULL CHECK (length(executable_value) BETWEEN 1 AND 2048),
    arguments_json TEXT NOT NULL CHECK (length(arguments_json) <= 131072 AND json_valid(arguments_json) AND json_type(arguments_json) = 'array' AND json_array_length(arguments_json) <= 128),
    working_directory_kind TEXT CHECK (working_directory_kind IN ('installationRelative','absolutePath')),
    working_directory_value TEXT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK ((working_directory_kind IS NULL AND working_directory_value IS NULL) OR (working_directory_kind IS NOT NULL AND working_directory_value IS NOT NULL AND length(working_directory_value) BETWEEN 1 AND 2048))
) STRICT;
CREATE INDEX ix_launch_profiles_installation ON launch_profiles(installation_id,id);

CREATE TABLE game_launch_defaults (
    game_id TEXT PRIMARY KEY NOT NULL REFERENCES games(id) ON DELETE RESTRICT,
    profile_id TEXT NOT NULL REFERENCES launch_profiles(id) ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER launch_profile_fixed_identity
BEFORE UPDATE OF id,installation_id,provenance ON launch_profiles
WHEN NEW.id <> OLD.id OR NEW.installation_id <> OLD.installation_id OR NEW.provenance <> OLD.provenance
BEGIN SELECT RAISE(ABORT, 'LaunchProfile identity and binding are immutable'); END;

CREATE TRIGGER launch_installation_fixed_parent
BEFORE UPDATE OF entry_id ON game_installations
WHEN NEW.entry_id <> OLD.entry_id AND EXISTS (SELECT 1 FROM launch_profiles WHERE installation_id=OLD.id)
BEGIN SELECT RAISE(ABORT, 'Remove LaunchProfiles before reassociation'); END;

CREATE TRIGGER launch_membership_fixed_parent
BEFORE UPDATE OF game_id ON library_entries
WHEN NEW.game_id <> OLD.game_id AND EXISTS (SELECT 1 FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id WHERE i.entry_id=OLD.id)
BEGIN SELECT RAISE(ABORT, 'Remove LaunchProfiles before reassociation'); END;

CREATE TRIGGER launch_default_same_game_insert
BEFORE INSERT ON game_launch_defaults
WHEN NOT EXISTS (SELECT 1 FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE p.id=NEW.profile_id AND e.game_id=NEW.game_id)
BEGIN SELECT RAISE(ABORT, 'Default LaunchProfile must belong to the same game'); END;

CREATE TRIGGER launch_default_same_game_update
BEFORE UPDATE ON game_launch_defaults
WHEN NOT EXISTS (SELECT 1 FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE p.id=NEW.profile_id AND e.game_id=NEW.game_id)
BEGIN SELECT RAISE(ABORT, 'Default LaunchProfile must belong to the same game'); END;
