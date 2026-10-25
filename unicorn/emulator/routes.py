# /emulator routes added to main.py's Microdot server.

import asyncio
import json
import os
import struct
import uuid

from microdot import send_file
from microdot.websocket import with_websocket

import runtime
import scenarios

PORT = int(os.environ.get("UNICORN_EMULATOR_PORT", "8080"))

EMULATOR_DIR = runtime.EMULATOR_DIR

# Changes on every restart (os.execv keeps the pid), so the page can tell a restart from a dropped connection
INSTANCE_ID = uuid.uuid4().hex

# State also changes without a new frame (pause, tune, an animation finishing), so it's rechecked at least this often
STATE_CHECK_SECONDS = 0.1

# Globals in main.py exposed as controls; set through main.py's own /tune route
TUNABLES = [
    {"var": "INBOX_CENTER", "type": "bool"},
    {"var": "INBOX_STAGGER", "type": "int", "min": 0, "max": 20, "step": 1, "unit": "px"},
    {"var": "INBOX_SLIDE_SPEED", "type": "int", "min": 1, "max": 10, "step": 1, "unit": "px/frame"},
    {"var": "INBOX_SHIFT_SPEED", "type": "int", "min": 1, "max": 10, "step": 1, "unit": "px/frame"},
    {"var": "INBOX_SLOT_WIDTH", "type": "int", "min": 8, "max": 16, "step": 1, "unit": "px"},
    {"var": "INBOX_Y", "type": "int", "min": -5, "max": 2, "step": 1, "unit": "px"},
    {"var": "MESSAGE_ICON_PUSH", "type": "bool"},
    {"var": "MESSAGE_ICON_TRIO", "type": "bool"},
    {"var": "MESSAGE_TRIO_STAGGER", "type": "int", "min": 0, "max": 12, "step": 1, "unit": "px"},
    {"var": "MESSAGE_TRIO_SPACING", "type": "int", "min": 0, "max": 13, "step": 1, "unit": "px"},
    {"var": "MESSAGE_TRIO_TEXT_SPACING", "type": "int", "min": 0, "max": 53, "step": 1, "unit": "px"},
    {"var": "MESSAGE_TRIO_PAUSE", "type": "int", "min": 0, "max": 100, "step": 5, "unit": "frames"},
    {"var": "MESSAGE_ICON_GAP", "type": "int", "min": 0, "max": 10, "step": 1, "unit": "px"},
    {"var": "DELAY_MILLIS", "type": "int", "min": 8, "max": 100, "step": 1, "unit": "ms/frame"},
    {"var": "HOLD_ENABLE", "type": "bool"},
    {"var": "HOLD_TIME", "type": "int", "min": 0, "max": 300, "step": 5, "unit": "frames"},
    {"var": "RAINBOW_FRAME_MULTIPLIER", "type": "int", "min": 0, "max": 10, "step": 1, "unit": ""},
    {"var": "LATE_FRAME_ENABLE", "type": "bool"},
]


def install(app):
    # Started first so the emulator still restarts on the next change if anything below fails
    asyncio.create_task(restart_on_change())

    @app.get("/emulator")
    async def emulator_page(request):
        return send_file(str(EMULATOR_DIR / "emulator.html"), max_age=0)

    @app.get("/emulator/state")
    async def emulator_state(request):
        return describe_state()

    @app.post("/emulator/clock")
    async def emulator_clock(request):
        body = request.json or {}
        if "speed" in body:
            runtime.clock.set_speed(float(body["speed"]))
        if "paused" in body:
            runtime.clock.set_paused(bool(body["paused"]))
        if body.get("step"):
            runtime.clock.step()
        return describe_state()

    @app.get("/emulator/history")
    async def emulator_history(request):
        return encode_history(), 200, {"Content-Type": "application/octet-stream"}

    @app.post("/emulator/seed")
    async def emulator_seed(request):
        body = request.json or {}
        scenarios.seed(int(body.get("count", 3)))
        return describe_state()

    @app.post("/emulator/scenario/<scenario_id>")
    async def emulator_scenario(request, scenario_id):
        scenario = scenarios.find(scenario_id)
        if scenario is None:
            return {"error": "unknown scenario"}, 404
        asyncio.create_task(scenario.run())
        return describe_state()

    @app.post("/emulator/restart")
    async def emulator_restart(request):
        asyncio.create_task(restart_soon())
        return {"restarting": True}

    @app.route("/emulator/live")
    @with_websocket
    async def emulator_live(request, ws):
        """Binary messages are frames; text messages are JSON: a hello first, then state whenever it changes."""
        await ws.send(json.dumps({"type": "hello", "instance": INSTANCE_ID, "config": describe_config()}))
        queue = runtime.frames.subscribe()
        sent_state = None
        try:
            while True:
                try:
                    await ws.send(await asyncio.wait_for(queue.get(), STATE_CHECK_SECONDS))
                except asyncio.TimeoutError:
                    pass
                state = json.dumps({"type": "state", **describe_state()})
                if state != sent_state:
                    await ws.send(state)
                    sent_state = state
        except ConnectionError:
            # The page closed or reloaded; a failed send is how this loop finds out
            pass
        finally:
            runtime.frames.unsubscribe(queue)

    TUNABLE_DEFAULTS.update({t["var"]: getattr(runtime.main, t["var"]) for t in available_tunables()})


TUNABLE_DEFAULTS = {}


def describe_config():
    main = runtime.main
    return {
        "width": main.WIDTH,
        "height": main.HEIGHT,
        "icons": sorted(main.ICONS),
        "tunables": [dict(t, default=TUNABLE_DEFAULTS[t["var"]]) for t in available_tunables()],
        "scenarios": [{"id": s.id, "label": s.label, "description": s.description} for s in scenarios.SCENARIOS],
    }


def describe_state():
    main = runtime.main
    return {
        "clock": {
            "ms": int(runtime.clock.now_ms()),
            "speed": runtime.clock.speed,
            "paused": runtime.clock.paused,
        },
        "frames": runtime.frames.count,
        "current": describe_animation_item(main.animation_item),
        "interrupting": main.animation_interrupt,
        "queue": [describe_animation_item(item) for item in sorted(main.animation_heap)],
        "inbox": [{"text": m.get("text"), "icon": m.get("icon")} for m in main.message_inbox],
        "tunables": {t["var"]: getattr(main, t["var"]) for t in available_tunables()},
    }


def describe_animation_item(item):
    if item is None:
        return None
    return {"name": item.animation.__name__, "priority": item.priority}


def available_tunables():
    """TUNABLES that main.py defines. One can be missing while main.py and this file are edited out of step."""
    return [t for t in TUNABLES if hasattr(runtime.main, t["var"])]


def encode_history():
    """Each frame as a little-endian uint32 frame number and uint32 virtual ms, then its RGB bytes."""
    return b"".join(struct.pack("<II", number, int(ms)) + frame for number, ms, frame in runtime.frames.history)


async def restart_on_change():
    """Re-exec the emulator when main.py or the emulator itself changes; the page reconnects on its own."""
    initial = runtime.source_mtimes()
    while True:
        await asyncio.sleep(0.5)
        if runtime.source_mtimes() != initial:
            print("[emulator] source changed, restarting")
            runtime.restart()


async def restart_soon():
    await asyncio.sleep(0.1)
    runtime.restart()
