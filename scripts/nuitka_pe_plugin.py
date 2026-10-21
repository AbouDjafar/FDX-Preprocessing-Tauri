"""Build-only adapter for Nuitka 4.3's pefile DLL scanner on Windows.

The legacy Dependency Walker fails on some recent Qt DLLs. pefile works, but its
explicit full garbage collections repeatedly walk the compiler's AST heap.
Keep the same import/delay-import analysis in a small cached subprocess instead.
No dependency package is modified and this adapter is not part of the application.
"""

import functools
import json
import subprocess
import sys
from pathlib import Path

from nuitka.containers.OrderedSets import OrderedSet
from nuitka.freezer import DllDependenciesWin32PEFile
from nuitka.plugins.PluginBase import NuitkaPluginBase
from nuitka.utils import SharedLibraries
from nuitka.utils.InlineCopies import importFromInlineCopy


class NuitkaPluginFDXPE(NuitkaPluginBase):
    plugin_name = "fdx-isolated-pe-scan"

    def __init__(self):
        pefile = importFromInlineCopy("pefile", must_exist=True)
        module_directory = str(Path(pefile.__file__).parent)
        worker = str(Path(__file__).with_name("pe_imports.py"))

        @functools.lru_cache(maxsize=None)
        def imports(filename):
            result = subprocess.run(
                [sys.executable, "-I", "-S", worker, module_directory, filename],
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                creationflags=subprocess.CREATE_NO_WINDOW,
                timeout=60,
                check=True,
            )
            values = json.loads(result.stdout)
            return OrderedSet(values) if values is not None else None

        SharedLibraries.getPEFileUsedDllNames = imports
        DllDependenciesWin32PEFile.getPEFileUsedDllNames = imports
