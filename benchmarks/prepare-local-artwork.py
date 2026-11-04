# SPDX-License-Identifier: GPL-3.0-or-later
"""Create only disposable synthetic test images, with no imaging dependency."""
import struct
import sys
import zlib
from pathlib import Path


def png(width, height, color):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    raw = (b'\0' + bytes(color) * width) * height
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')


if __name__ == '__main__':
    root = Path(sys.argv[1]).resolve(strict=True)
    for name, color in [('first.png', (42, 130, 210)), ('second.png', (180, 70, 120))]:
        with (root / name).open('xb') as file:
            file.write(png(1280, 720, color))
    with (root / 'corrupt.png').open('xb') as file:
        file.write(b'deliberately invalid disposable artwork')
