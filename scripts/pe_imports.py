"""Read DLL imports in a small process, isolated from Nuitka's large compiler heap."""

import importlib
import json
import sys

sys.path.insert(0, sys.argv[1])
pefile = importlib.import_module("pefile")
try:
    with pefile.PE(sys.argv[2], fast_load=True) as image:
        image.parse_data_directories(
            directories=[
                pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_IMPORT"],
                pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT"],
            ]
        )
        names = list(
            dict.fromkeys(
                entry.dll.decode("utf-8")
                for directory in ("DIRECTORY_ENTRY_IMPORT", "DIRECTORY_ENTRY_DELAY_IMPORT")
                for entry in getattr(image, directory, ())
            )
        )
except pefile.PEFormatError:
    names = None
print(json.dumps(names))
