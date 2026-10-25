# Scripted cases that drive main.py's route handlers on the virtual clock.

import asyncio
import inspect
from collections import namedtuple

import runtime


Scenario = namedtuple("Scenario", "id label description run")


async def one_message():
    await call("message", json=message("Lorem ipsum dolor", icon="calendar"))


async def burst_of_three():
    await call("message", json=message("Lorem ipsum", icon="calendar"))
    await call("message", json=message("Dolor sit amet", icon="flag"))
    await call("message", json=message("Consectetur", icon=None))


async def message_mid_slide():
    seed(3)
    await until_playing("inbox_animation")
    await wait(200)
    await call("message", json=message("Interrupting", icon="timer"))


async def full_inbox():
    seed(5)
    await until_playing("inbox_animation")
    await wait(500)
    await call("message", json=message("One too many", icon="clock"))


async def read_inbox():
    seed(2)
    await until_playing("inbox_animation")
    await wait(500)
    await call("read_inbox")


async def clear_mid_slide():
    seed(4)
    await until_playing("inbox_animation")
    await wait(300)
    await call("clear_inbox")


async def countdown_interrupted():
    await call("countdown", args={"seconds": "5", "icon": "timer"})
    await wait(1500)
    await call("message", json=message("Lorem ipsum", icon="flag"))


async def replay_slide_in():
    runtime.main.enqueue_animation(runtime.main.inbox_animation(), priority=3)


SCENARIOS = [
    Scenario("one", "One message", "Message rises, holds, scrolls off, then its icon slides in.", one_message),
    Scenario("burst", "Burst of 3", "Three arrive together; all play, then icons slide up from the bottom. Last has no icon (envelope).", burst_of_three),
    Scenario("mid-slide", "Message mid-slide", "Seed 3, interrupt the slide with a new message.", message_mid_slide),
    Scenario("full", "Full inbox", "Seed 5 (the last is clipped), then a 6th arrives and is not drawn.", full_inbox),
    Scenario("read", "Read inbox", "Seed 2, then read: icons vanish and messages replay.", read_inbox),
    Scenario("clear-mid-slide", "Clear mid-slide", "Seed 4, clear while sliding.", clear_mid_slide),
    Scenario("countdown", "Countdown interrupted", "5s countdown interrupted by a message, then resumes.", countdown_interrupted),
    Scenario("replay", "Replay slide-in", "Re-slide the current inbox from the bottom.", replay_slide_in),
]


def find(scenario_id):
    return next((s for s in SCENARIOS if s.id == scenario_id), None)


def seed(count):
    """Replace the inbox without playing message animations, then (re)start the inbox animation."""
    main = runtime.main
    icons = [*sorted(main.ICONS), None]
    main.message_inbox[:] = [
        message("Seeded {}".format(i + 1), icon=icons[i % len(icons)]) for i in range(count)
    ]
    main.enqueue_animation(main.inbox_animation(), priority=3)


def message(text, icon=None, rainbow=False, read=False):
    return {"text": text, "icon": icon, "effects": ["rainbow"] if rainbow else [], "read": read}


async def call(route, args=None, json=None):
    result = getattr(runtime.main, route)(Request(args or {}, json))
    if inspect.isawaitable(result):
        await result


async def wait(ms):
    await runtime.clock.sleep_ms(ms)


async def until_playing(name):
    while True:
        item = runtime.main.animation_item
        if item is not None and item.animation.__name__ == name:
            return
        await asyncio.sleep(0.01)


class Request:
    def __init__(self, args, json):
        self.args = args
        self.json = json
