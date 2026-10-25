"""Run unicorn/src/main.py on the desktop, with a browser page to view and drive it.

    unicorn/.venv/bin/python unicorn/emulator/emulator.py
    open http://localhost:8080/emulator

Hardware and MicroPython-only modules come from emulator/shims. Where main.py imports a module
that also exists in CPython (time, asyncio, microdot), it gets the emulator variant instead.
"""

import builtins
import importlib.util
import os
import sys
import time
import traceback
import types
from pathlib import Path

EMULATOR_DIR = Path(__file__).resolve().parent
SRC_DIR = EMULATOR_DIR.parent / "src"

MAIN_MODULE = "main"

OVERRIDES = {
    "time": "vtime",
    "asyncio": "vasyncio",
    "microdot": "vmicrodot",
}


def run():
    # Absolute so routes.restart() can re-exec after the chdir below
    sys.argv[0] = str(Path(__file__).resolve())
    sys.path[:0] = [str(EMULATOR_DIR / "shims"), str(EMULATOR_DIR), str(SRC_DIR)]
    os.chdir(SRC_DIR)

    builtins.micropython = types.SimpleNamespace(
        native=lambda f: f,
        viper=lambda f: f,
        const=lambda value: value,
    )
    builtins.__import__ = overriding_import(builtins.__import__)

    import runtime

    spec = importlib.util.spec_from_file_location(MAIN_MODULE, SRC_DIR / "main.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[MAIN_MODULE] = module
    runtime.main = module

    initial = runtime.source_mtimes()
    try:
        # main.py ends with asyncio.run(main()), so this only returns on shutdown
        spec.loader.exec_module(module)
    except Exception:
        # Usually main.py mid-edit, e.g. a syntax error; wait for the next save instead of exiting
        traceback.print_exc()
        print("[emulator] main.py failed, restarting when sources change", flush=True)
        while runtime.source_mtimes() == initial:
            time.sleep(0.5)
        runtime.restart()


def overriding_import(original_import):
    def hooked_import(name, globals=None, locals=None, fromlist=(), level=0):
        if level == 0 and name in OVERRIDES and globals is not None and globals.get("__name__") == MAIN_MODULE:
            name = OVERRIDES[name]
        return original_import(name, globals, locals, fromlist, level)

    return hooked_import


if __name__ == "__main__":
    try:
        run()
    except KeyboardInterrupt:
        # Ctrl-C: exit without asyncio.run's traceback, using the conventional SIGINT exit status
        sys.exit(130)
