# SPDX-License-Identifier: GPL-3.0-or-later
"""Create an application-purpose synthetic copy; never open the source for writes."""
import os
from pathlib import Path
import sqlite3
import sys

source, destination = (Path(p).resolve() for p in sys.argv[1:])
if source == destination:
    raise ValueError("Choose a new destination")
with sqlite3.connect(source.as_uri() + "?mode=ro", uri=True) as original:
    if original.execute("SELECT purpose FROM database_metadata").fetchone() != ("benchmark",):
        raise ValueError("Expected a synthetic benchmark source")
    handle = os.open(destination, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    os.close(handle)
    with sqlite3.connect(destination) as copy:
        original.backup(copy)
        copy.execute("UPDATE database_metadata SET purpose='application'")
