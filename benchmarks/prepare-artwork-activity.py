# SPDX-License-Identifier: GPL-3.0-or-later
"""Attach synthetic artwork to a stopped, migrated, disposable application copy."""
import sqlite3
import runpy
import sys
from pathlib import Path
png = runpy.run_path(str(Path(__file__).with_name('prepare-local-artwork.py')))['png']

if __name__ == '__main__':
    database = Path(sys.argv[1]).resolve(strict=True)
    # Only runner-owned application copies under an ignored results directory.
    if 'results' not in database.parts or 'application' not in database.parts:
        raise ValueError('Expected an isolated application copy under results')
    with sqlite3.connect(database) as connection:
        assert connection.execute('SELECT purpose FROM database_metadata').fetchone() == ('application',)
        assert connection.execute('SELECT max(version) FROM _sqlx_migrations').fetchone() == (5,)
        connection.execute('PRAGMA foreign_keys=ON')
        originals = database.parent / 'artwork' / 'originals'
        originals.mkdir(parents=True, exist_ok=False)
        content = png(1280, 720, (30, 120, 180))
        fingerprint = 0xcbf29ce484222325
        for value in content:
            fingerprint = ((fingerprint ^ value) * 0x100000001b3) & ((1 << 64) - 1)
        ids = connection.execute('SELECT id FROM games ORDER BY sort_title,id LIMIT 270').fetchall()
        for index, (game,) in enumerate(ids):
            asset = f'{index % 256:02x}{index:030x}'
            with (originals / f'{asset}.img').open('xb') as file:
                file.write(content)
            connection.execute("UPDATE games SET origin='manual' WHERE id=?", (game,))
            connection.execute('INSERT INTO artwork_assets VALUES (?,?,?,?,?,?)',
                               (asset, 'png', len(content), 1280, 720, f'{fingerprint:016x}'))
            connection.execute('INSERT INTO game_artwork VALUES (?,?)', (game, asset))
        # A larger selected source exercises a bounded 4M-pixel import while browsing.
        with (database.parent.parent.parent / 'import.png').open('xb') as file:
            file.write(png(2048, 2048, (190, 70, 40)))
