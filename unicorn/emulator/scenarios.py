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


async def message_with_ttl():
    await call("message", json=message("Lorem ipsum", icon="cat", ttl=8))


async def expire_middle():
    seed(3, ttls={1: 3})


async def expire_with_overflow():
    seed(6, ttls={1: 3})


async def add_in_place():
    seed(2)
    await until_playing("inbox_animation")
    await wait(2000)
    # Appended without a message animation, which would restart the inbox
    runtime.main.message_inbox.append(message("Lorem ipsum", icon="siren"))


async def remove_by_id():
    seed(2)
    await call("message", json=message("Lorem ipsum +2", icon="siren", id="datadog"))
    await until_in_inbox("datadog")
    await wait(1500)
    await call("remove_message", args={"id": "datadog"})


async def replace_by_id():
    await call("message", json=message("Lorem ipsum", icon="siren", id="datadog"))
    await until_in_inbox("datadog")
    await wait(1000)
    await call("message", json=message("Dolor sit amet", icon="siren", id="datadog"))


async def replay_slide_in():
    # No id, so it queues even while an inbox is running
    runtime.main.enqueue_animation(runtime.main.inbox_animation(), priority=3)


async def clock_then_message():
    await start_clock()
    await call("message", json=message("Lorem ipsum", icon="calendar"))


async def clock_then_countdown():
    await start_clock()
    await call("countdown", args={"seconds": "5", "icon": "timer"})


async def clock_then_clear():
    await start_clock()
    await call("clear_inbox")


async def clock_behind_inbox():
    seed(2)
    await until_playing("inbox_animation")
    await wait(1500)
    await call("start_clock", args={"start_timestamp": str(CLOCK_START_TIMESTAMP)})


async def inbox_then_message():
    await settled_inbox()
    await call("message", json=message("Lorem ipsum", icon="flag"))


async def inbox_then_countdown():
    await settled_inbox()
    await call("countdown", args={"seconds": "5", "icon": "timer"})


async def countdown_then_cancel():
    await call("countdown", args={"seconds": "30", "icon": "timer"})
    await until_playing("countdown_animation")
    await wait(1500)
    await call("cancel_animation")


async def countdown_finishes():
    seed(2)
    await call("countdown", args={"seconds": "3", "icon": "timer"})


async def start_clock():
    """Starts the clock and waits until it has dropped in."""
    await call("start_clock", args={"start_timestamp": str(CLOCK_START_TIMESTAMP)})
    await until_playing("clock_animation")
    await wait(1500)


async def settled_inbox():
    """Seeds 3 and waits until their icons have risen into place."""
    seed(3)
    await until_playing("inbox_animation")
    await wait(2500)


# 10:00, the clock's seconds since midnight
CLOCK_START_TIMESTAMP = 10 * 3600


SCENARIOS = [
    Scenario("one", "One message", "Message rises, holds, scrolls off, then its icon slides in.", one_message),
    Scenario("burst", "Burst of 3", "Three arrive together; all play, then icons slide up from the bottom. Last has no icon (envelope).", burst_of_three),
    Scenario("mid-slide", "Message mid-slide", "Seed 3, interrupt the slide with a new message.", message_mid_slide),
    Scenario("full", "Full inbox", "Seed 5 (the last is clipped), then a 6th arrives and is not drawn.", full_inbox),
    Scenario("read", "Read inbox", "Seed 2, then read: the first message pushes the row up and both replay.", read_inbox),
    Scenario("clear-mid-slide", "Clear mid-slide", "Seed 4, clear while sliding: the icons sink out.", clear_mid_slide),
    Scenario("countdown", "Countdown interrupted", "5s countdown interrupted by a message, then resumes.", countdown_interrupted),
    Scenario("ttl", "Message with TTL", "8s TTL from arrival: message plays, its icon slides in, then drops out.", message_with_ttl),
    Scenario("expire-middle", "Expire middle", "Seed 3, the middle with a 3s TTL: it sinks, then the last shifts left.", expire_middle),
    Scenario("expire-overflow", "Expire with overflow", "Seed 6, the 2nd with a 3s TTL: after it sinks the rest shift left and the 6th slides in from the right.", expire_with_overflow),
    Scenario("add", "Add in place", "Seed 2, then a 3rd is added without playing: the row shifts to make room, then it rises.", add_in_place),
    Scenario("remove", "Remove by id", "Seed 2, a siren with id datadog arrives, then is removed: it drops out like an expiry.", remove_by_id),
    Scenario("replace", "Replace by id", "A siren with id datadog arrives, then another with the same id replaces it quietly, without playing.", replace_by_id),
    Scenario("replay", "Replay slide-in", "Re-slide the current inbox from the bottom.", replay_slide_in),
    Scenario("clock-message", "Clock → message", "The clock drops in, then a message pushes it up. The clock doesn't come back.", clock_then_message),
    Scenario("clock-countdown", "Clock → countdown", "The clock drops in, then a 5s countdown drops in and pushes it out the bottom. The clock doesn't come back.", clock_then_countdown),
    Scenario("clock-clear", "Clock → clear inbox", "The clock drops in, then clearing the (empty) inbox interrupts it: the clock is pushed up and the display is left blank.", clock_then_clear),
    Scenario("clock-behind-inbox", "Clock behind inbox", "Seed 2, then start the clock: it waits until the inbox empties.", clock_behind_inbox),
    Scenario("inbox-message", "Inbox → message", "Seed 3 and let them settle, then a message pushes the row up.", inbox_then_message),
    Scenario("inbox-countdown", "Inbox → countdown", "Seed 3 and let them settle, then a 5s countdown drops in and pushes the row out the bottom.", inbox_then_countdown),
    Scenario("countdown-cancel", "Countdown → cancel", "A 30s countdown drops in, then is cancelled: it switches off in the SWITCH_OFF_STYLE.", countdown_then_cancel),
    Scenario("countdown-end", "Countdown finishes", "Seed 2, then a 3s countdown drops in, runs out, drops away, and the inbox rises.", countdown_finishes),
]


def find(scenario_id):
    return next((s for s in SCENARIOS if s.id == scenario_id), None)


def seed(count, ttls=None):
    """Replace the inbox without playing message animations, then start the inbox animation if none is running.

    ttls maps an inbox index to seconds until it expires, stamped as the /message route would.
    """
    main = runtime.main
    icons = [*sorted(main.ICONS), None]
    main.message_inbox[:] = [
        message("Seeded {}".format(i + 1), icon=icons[i % len(icons)]) for i in range(count)
    ]
    for i, ttl in (ttls or {}).items():
        main.message_inbox[i]["expires_ms"] = main.time.ticks_add(main.time.ticks_ms(), ttl * 1000)
    main.enqueue_inbox_animation()


def message(text, icon=None, rainbow=False, read=False, ttl=None, id=None):
    return {"text": text, "icon": icon, "effects": ["rainbow"] if rainbow else [], "read": read, "ttl": ttl, "id": id}


async def call(route, args=None, json=None):
    result = getattr(runtime.main, route)(Request(args or {}, json))
    if inspect.isawaitable(result):
        await result


async def wait(ms):
    await runtime.clock.sleep_ms(ms)


async def until_in_inbox(id):
    while not any(m.get("id") == id for m in runtime.main.message_inbox):
        await asyncio.sleep(0.01)


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
