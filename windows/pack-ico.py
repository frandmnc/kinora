# Junta los PNG de windows/assets en kinora.ico (un .ico admite PNG adentro desde Windows Vista).
# Uso: python3 windows/pack-ico.py windows/assets
import struct, sys, pathlib
folder = pathlib.Path(sys.argv[1])
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [(folder / f"kinora-{s}.png").read_bytes() for s in sizes]
header = struct.pack("<HHH", 0, 1, len(sizes))
offset = 6 + 16 * len(sizes)
entries = b""
for size, data in zip(sizes, images):
    side = 0 if size == 256 else size
    entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
(folder / "kinora.ico").write_bytes(header + entries + b"".join(images))
