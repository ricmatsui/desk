# microdot for main.py: serves on the emulator port and adds the /emulator routes.

import inspect

import microdot
from microdot import send_file  # noqa: F401

import routes

# Browsers send long localhost cookie headers; the device never sees these
microdot.Request.max_readline = 16 * 1024


async def invoke_handler(handler, *args, **kwargs):
    """Run sync handlers on the event loop, as MicroPython's microdot does.

    CPython's microdot runs them in a thread pool instead, where main.py's sync routes (/stop, /cancel-animation)
    race the animation globals and fail to start run_animations() with "no running event loop".
    """
    ret = handler(*args, **kwargs)
    if inspect.isawaitable(ret):
        ret = await ret
    return ret


microdot.microdot.invoke_handler = invoke_handler


class Microdot(microdot.Microdot):
    async def start_server(self, host="0.0.0.0", port=80, debug=False, ssl=None):
        routes.install(self)
        print("[emulator] http://localhost:{}/emulator".format(routes.PORT))
        return await super().start_server(host=host, port=routes.PORT, debug=False, ssl=ssl)
