# machine shim: reset() is logged instead of rebooting; use the emulator Restart button to reload.


def reset():
    print("[emulator] machine.reset() ignored")
