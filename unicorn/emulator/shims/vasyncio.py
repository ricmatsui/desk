# asyncio for main.py: adds MicroPython's sleep_ms on the emulator's virtual clock.

from asyncio import *  # noqa: F401,F403

import runtime


def sleep_ms(ms):
    return runtime.clock.sleep_ms(ms)
