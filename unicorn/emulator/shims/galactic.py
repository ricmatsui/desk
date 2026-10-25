# Galactic Unicorn shim: update() publishes the framebuffer to the emulator instead of the LEDs.

import runtime


class GalacticUnicorn:
    def __init__(self):
        self._brightness = 0.5

    def set_brightness(self, brightness):
        self._brightness = brightness

    def get_brightness(self):
        return self._brightness

    def update(self, graphics):
        # Framebuffer is B, G, R, 0 per pixel; frames are sent as R, G, B
        buffer = bytes(graphics)
        frame = bytearray(len(buffer) // 4 * 3)
        frame[0::3] = buffer[2::4]
        frame[1::3] = buffer[1::4]
        frame[2::3] = buffer[0::4]
        runtime.frames.publish(bytes(frame), runtime.clock.now_ms())
        runtime.clock.frame_presented()
