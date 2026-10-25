# Shared state between the shims, the emulator routes and the loaded main.py.

import asyncio
import time

# The loaded main.py module, set by emulator.py before it executes
main = None


class VirtualClock:
    """Clock behind time.ticks_ms/time.time/asyncio.sleep_ms that can be paused, stepped and scaled."""

    def __init__(self):
        self.speed = 1.0
        self.paused = False
        self.stepping = False
        self.epoch = int(time.time())
        self._anchor_real = time.monotonic()
        self._anchor_virtual_ms = 0.0

    def now_ms(self):
        if self.paused:
            return self._anchor_virtual_ms
        return self._anchor_virtual_ms + (time.monotonic() - self._anchor_real) * 1000 * self.speed

    def set_speed(self, speed):
        self._reanchor()
        self.speed = max(0.01, speed)

    def set_paused(self, paused):
        self._reanchor()
        self.paused = paused
        self.stepping = False

    def step(self):
        """Run until the next frame is presented, then pause."""
        self._reanchor()
        self.paused = False
        self.stepping = True

    def frame_presented(self):
        if self.stepping:
            self.set_paused(True)

    async def sleep_ms(self, ms):
        target_ms = self.now_ms() + ms
        while True:
            remaining_ms = target_ms - self.now_ms()
            if remaining_ms <= 0:
                return
            if self.paused:
                await asyncio.sleep(0.02)
            else:
                await asyncio.sleep(min(remaining_ms / 1000 / self.speed, 0.05))

    def _reanchor(self):
        self._anchor_virtual_ms = self.now_ms()
        self._anchor_real = time.monotonic()


class FrameBus:
    """Fans out presented frames (WIDTH * HEIGHT * RGB bytes) to websocket subscribers, dropping stale ones."""

    def __init__(self):
        self.latest = None
        self.count = 0
        self._subscribers = set()

    def publish(self, frame):
        self.latest = frame
        self.count += 1
        for queue in self._subscribers:
            # Each queue holds one frame. A subscriber that hasn't sent the previous frame yet gets it replaced,
            # so a slow websocket skips frames instead of buffering a backlog or blocking the animation loop.
            if queue.full():
                queue.get_nowait()
            queue.put_nowait(frame)

    def subscribe(self):
        queue = asyncio.Queue(maxsize=1)
        if self.latest is not None:
            queue.put_nowait(self.latest)
        self._subscribers.add(queue)
        return queue

    def unsubscribe(self, queue):
        self._subscribers.discard(queue)


clock = VirtualClock()
frames = FrameBus()
