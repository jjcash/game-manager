# SPDX-License-Identifier: GPL-3.0-or-later
"""Consistent standalone wrong-purpose fixture; never copy a live main file alone."""
import os
from pathlib import Path
import sqlite3
import sys

source, destination = (Path(value).resolve() for value in sys.argv[1:])
if source == destination or not destination.parent.is_dir():
    raise ValueError("Use a new destination inside the disposable smoke root")
with sqlite3.connect(source.as_uri() + "?mode=ro", uri=True) as original:
    if original.execute("SELECT purpose FROM database_metadata").fetchone() != ("benchmark",):
        raise ValueError("Expected a synthetic benchmark source")
    handle = os.open(destination, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    os.close(handle)
    with sqlite3.connect(destination) as snapshot:
        original.backup(snapshot)
        assert snapshot.execute("PRAGMA integrity_check").fetchone() == ("ok",)
        snapshot.execute("PRAGMA journal_mode=DELETE")
for suffix in ("-wal", "-shm", "-journal"):
    if Path(str(destination) + suffix).exists():
        raise ValueError("Fixture is not standalone; preserve output for inspection")
