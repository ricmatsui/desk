# time for main.py: MicroPython ticks API on the emulator's virtual clock.

from time import sleep, localtime, gmtime  # noqa: F401

import runtime


def ticks_ms():
    return int(runtime.clock.now_ms())


def ticks_diff(end, start):
    return end - start


def ticks_add(ticks, delta):
    return ticks + delta


def time():
    return runtime.clock.epoch + int(runtime.clock.now_ms() // 1000)
