"""Generate the small application mark in CI, using only Python's standard library."""
import struct
import zlib
from pathlib import Path


def png(size):
    def chunk(kind, value):
        return struct.pack('!I', len(value)) + kind + value + struct.pack('!I', zlib.crc32(kind + value))
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            u, v = x / size, y / size
            mark = (.29 <= u < .40 and .23 <= v < .79) or (.29 <= u < .74 and .23 <= v < .34) or (.29 <= u < .65 and .46 <= v < .57)
            rows.extend((230, 189, 116, 255) if mark else (25, 58, 53, 255))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(bytes(rows), 9)) + chunk(b'IEND', b'')


if __name__ == '__main__':
    destination = Path(__file__).resolve().parents[1] / 'src-tauri' / 'icons'
    destination.mkdir(parents=True, exist_ok=True)
    for size in (32, 128):
        (destination / f'{size}x{size}.png').write_bytes(png(size))
    images = [png(size) for size in (32, 48, 256)]
    offset = 6 + 16 * len(images)
    directory = bytearray(struct.pack('<3H', 0, 1, len(images)))
    for size, data in zip((32, 48, 256), images):
        directory.extend(struct.pack('<4B2H2I', size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (destination / 'icon.ico').write_bytes(bytes(directory) + b''.join(images))
