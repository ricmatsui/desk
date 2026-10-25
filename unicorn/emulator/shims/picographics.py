# Subset of Pimoroni PicoGraphics used by main.py, for the Galactic Unicorn (RGB888).
#
# The framebuffer matches the device: one little-endian uint32 0x00RRGGBB per pixel,
# so bytes are B, G, R, 0. main.py reads it through memoryview(graphics).
#
# Bitmap font rendering follows libraries/bitmap_fonts/bitmap_fonts.cpp in pimoroni-pico.

import colorsys

DISPLAY_GALACTIC_UNICORN = "galactic_unicorn"

WIDTH = 53
HEIGHT = 11

BASE_CHARS = 96
EXTRA_CHARS = 9

# Firmware up to v1.25.0 counts the spacing after the final character in measure_text; v1.27.0 onwards
# drops it (pimoroni-pico 6628efed). The device runs the older firmware, and main.py lays out text with it.
MEASURE_TRAILING_SPACING = True


class PicoGraphics(bytearray):
    def __init__(self, display):
        super().__init__(WIDTH * HEIGHT * 4)
        self._pen = 0
        self._font = None

    def get_bounds(self):
        return WIDTH, HEIGHT

    def create_pen(self, r, g, b):
        return (int(r) & 0xFF) << 16 | (int(g) & 0xFF) << 8 | (int(b) & 0xFF)

    def create_pen_hsv(self, h, s, v):
        r, g, b = colorsys.hsv_to_rgb(h % 1.0, s, v)
        return self.create_pen(r * 255, g * 255, b * 255)

    def set_pen(self, pen):
        self._pen = pen

    def set_font(self, font):
        if not isinstance(font, (bytes, bytearray)):
            raise NotImplementedError("emulator only supports custom bytearray fonts, got {!r}".format(font))
        self._font = BitmapFont(font)

    def clear(self):
        self[:] = self._pen_bytes() * (WIDTH * HEIGHT)

    def pixel(self, x, y):
        x = int(x)
        y = int(y)
        if 0 <= x < WIDTH and 0 <= y < HEIGHT:
            start = (y * WIDTH + x) * 4
            self[start:start + 4] = self._pen_bytes()

    def line(self, x1, y1, x2, y2):
        x1, y1, x2, y2 = int(x1), int(y1), int(x2), int(y2)
        dx = abs(x2 - x1)
        dy = -abs(y2 - y1)
        sx = 1 if x1 < x2 else -1
        sy = 1 if y1 < y2 else -1
        error = dx + dy
        while True:
            self.pixel(x1, y1)
            if x1 == x2 and y1 == y2:
                return
            e2 = 2 * error
            if e2 >= dy:
                error += dy
                x1 += sx
            if e2 <= dx:
                error += dx
                y1 += sy

    def text(self, text, x, y, wordwrap=0x7FFFFFFF, scale=None, angle=0, spacing=1, fixed_width=False, rotation=0):
        if angle or rotation:
            raise NotImplementedError("emulator does not support rotated text")
        scale = 2 if scale is None else int(scale)
        self._font.draw(self, str(text), int(x), int(y), scale, spacing, fixed_width)

    def measure_text(self, text, scale=None, spacing=1, fixed_width=False):
        scale = 2 if scale is None else int(scale)
        return self._font.measure(str(text), scale, spacing, fixed_width)

    def rectangle(self, x, y, w, h):
        for py in range(int(y), int(y) + int(h)):
            for px in range(int(x), int(x) + int(w)):
                self.pixel(px, py)

    def _pen_bytes(self):
        pen = self._pen
        return bytes((pen & 0xFF, (pen >> 8) & 0xFF, (pen >> 16) & 0xFF, 0))


class BitmapFont:
    """Layout: height, max_width, widths[BASE_CHARS + EXTRA_CHARS], then column-major glyph data."""

    def __init__(self, data):
        self.height = data[0]
        self.max_width = data[1]
        self.widths = data[2:2 + BASE_CHARS + EXTRA_CHARS]
        self.glyphs = data[2 + BASE_CHARS + EXTRA_CHARS:]
        self.two_bytes_per_column = self.height > 8
        self.bytes_per_char = self.max_width * (2 if self.two_bytes_per_column else 1)

    def draw(self, graphics, text, x, y, scale, spacing, fixed_width):
        char_offset = 0
        line_offset = 0
        for c in text:
            if c == "\n":
                char_offset = 0
                line_offset += (self.height + 1) * scale
            elif c == " ":
                char_offset += self.measure_character(c, scale, fixed_width) + spacing * scale
            else:
                self.draw_character(graphics, c, x + char_offset, y + line_offset, scale)
                char_offset += self.measure_character(c, scale, fixed_width) + spacing * scale

    def measure(self, text, scale, spacing, fixed_width):
        width = 0
        for c in text:
            width += self.measure_character(c, scale, fixed_width) + spacing * scale
        if width > 0 and not MEASURE_TRAILING_SPACING:
            width -= spacing * scale
        return width

    def measure_character(self, c, scale, fixed_width):
        index = ord(c) - 32
        if index < 0 or index >= BASE_CHARS:
            return 0
        if fixed_width:
            return self.max_width * scale
        return self.widths[index] * scale

    def draw_character(self, graphics, c, x, y, scale):
        index = ord(c) - 32
        if index < 0 or index >= BASE_CHARS:
            return
        start = index * self.bytes_per_char
        for cx in range(self.widths[index]):
            if self.two_bytes_per_column:
                column = self.glyphs[start + cx * 2] << 8 | self.glyphs[start + cx * 2 + 1]
            else:
                column = self.glyphs[start + cx]
            for bit in range(16):
                if column & (1 << bit):
                    graphics.rectangle(x + cx * scale, y + bit * scale, scale, scale)
