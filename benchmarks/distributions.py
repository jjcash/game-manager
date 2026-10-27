# SPDX-License-Identifier: GPL-3.0-or-later
"""New disposable SQL distributions; never alter the seed-42 reference file.

Python SQLite performs backup/generation only. Measure with library-bench's
bundled SQLx engine. Output metadata explicitly identifies the derivative.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("source", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("variant", choices=["sparse", "dense", "fanout"])
args = parser.parse_args()
source, output = args.source.resolve(strict=True), args.output.absolute()
if not args.source.is_absolute() or not args.output.is_absolute():
    parser.error("Explicit absolute fixture paths required")
if output.parent.name != "data" or output.parent.parent.name != "benchmarks":
    parser.error("Output must be a new file under an explicit benchmarks/data root")
digest = hashlib.sha256(source.read_bytes()).hexdigest()
with sqlite3.connect(source.as_uri() + "?mode=ro", uri=True) as original:
    metadata = original.execute("SELECT purpose,generation_complete,dataset_games FROM database_metadata").fetchone()
    if metadata[0:2] != ("benchmark", 1) or metadata[2] not in (15000, 25000):
        parser.error("Expected complete release/stress synthetic fixture")
    output.parent.mkdir(parents=True, exist_ok=True)
    if any(Path(str(output) + suffix).exists() for suffix in ("", "-wal", "-shm", "-journal")):
        parser.error("Output or sidecar already exists; preserved")
    output.touch(exist_ok=False)
    with sqlite3.connect(output) as db:
        original.backup(db)
        db.execute("PRAGMA foreign_keys=ON")
        games = [r[0] for r in db.execute("SELECT id FROM games ORDER BY id")]
        if args.variant in ("sparse", "dense"):
            # One copy per canonical game, deterministic 10%/90% installed.
            db.execute("DELETE FROM game_installations")
            selected = [(g, db.execute("SELECT id FROM library_entries WHERE game_id=? ORDER BY id LIMIT 1", (g,)).fetchone()[0]) for i, g in enumerate(games) if (i % 10 == 0 if args.variant == "sparse" else i % 10 != 0)]
            db.executemany("INSERT INTO game_installations(id,entry_id,root_path_ref,platform,detection_state) VALUES(?,?,?,'synthetic','present')", [("distribution-copy-" + g, e, "synthetic://distribution/" + g) for g, e in selected])
        else:
            # Six extra memberships and four copies per new entry. Existing
            # canonical titles/search and original entry/copy state are retained.
            for n in range(6):
                sid = f"distribution-source-{n}"
                db.execute("INSERT INTO library_sources VALUES(?, 'synthetic', ?)", (sid, sid))
                db.executemany("INSERT INTO library_entries(id,game_id,source_id,added_at_ms) VALUES(?,?,?,0)", [(f"distribution-entry-{n}-{g}", g, sid) for g in games])
                for copy in range(4):
                    db.executemany("INSERT INTO game_installations(id,entry_id,root_path_ref,platform,detection_state) VALUES(?,?,?,'synthetic',?)", [(f"distribution-copy-{n}-{copy}-{g}", f"distribution-entry-{n}-{g}", f"synthetic://distribution/{n}/{copy}/{g}", "present" if i % 10 else "missing") for i, g in enumerate(games)])
        assert db.execute("PRAGMA integrity_check").fetchone() == ("ok",)
        assert not db.execute("PRAGMA foreign_key_check").fetchall()
        counts = {table: db.execute(f"SELECT count(*) FROM {table}").fetchone()[0] for table in ("games", "library_entries", "game_installations", "library_sources")}
assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
manifest = {"variant": args.variant, "referenceSource": str(source), "referenceSha256": digest, "output": str(output), "counts": counts, "note": "Derivative distribution, not generator-v1 seed-42 reference evidence. No schema/index/ANALYZE changes."}
with Path(str(output) + ".distribution.json").open("x", encoding="utf-8") as f:
    json.dump(manifest, f, indent=2)
print(json.dumps(manifest))
