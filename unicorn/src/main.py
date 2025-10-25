from microdot import Microdot, send_file # pyright: ignore
from phew import connect_to_wifi # pyright: ignore
from galactic import GalacticUnicorn
from picographics import PicoGraphics, DISPLAY_GALACTIC_UNICORN as DISPLAY
from WIFI_CONFIG import SSID, PSK
from font import font
import asyncio
import deflate
import io
import math
import time
import machine
import ntptime
import heapq
import json
import random
import requests

DELAY_MILLIS = 24

HOLD_ENABLE = True
HOLD_TIME = 60
TEXT = "The quick brown fox jumps over the lazy dog."
RAINBOW_TIMER_THRESHOLD = 10
RAINBOW_FRAME_MULTIPLIER = 2

unicorn = GalacticUnicorn()
graphics = PicoGraphics(DISPLAY)
graphics_view = memoryview(graphics) # pyright: ignore
unicorn.set_brightness(0.5)

# 53x11
WIDTH, HEIGHT = graphics.get_bounds()

def get_pixel(x, y):
    start = (y * WIDTH + x) * 4
    return tuple(graphics_view[start:start+3])

WHITE = graphics.create_pen(255, 255, 255)
YELLOW = graphics.create_pen(255, 255, 0)
BLUE = graphics.create_pen(0, 0, 255)
GREEN = graphics.create_pen(0, 255, 0)
RED = graphics.create_pen(255, 0, 0)
BLACK = graphics.create_pen(0, 0, 0)

graphics.set_pen(YELLOW)
graphics.pixel(0, 0)
unicorn.update(graphics)

ip = connect_to_wifi(SSID, PSK)

graphics.set_pen(BLUE)
graphics.pixel(0, 0)
unicorn.update(graphics)

server = Microdot()

class AnimationItem:
    _counter = 0
    
    def __init__(self, priority, animation):
        self.priority = priority
        self.animation = animation
        AnimationItem._counter += 1
        self._order = AnimationItem._counter
    
    def __lt__(self, other):
        if self.priority != other.priority:
            return self.priority < other.priority
        return self._order < other._order

animation_heap = []
animation_priority = None
animation_running = False
animation_interrupt = False

def enqueue_animation(animation, priority):
    global animation_heap
    global animation_priority
    global animation_running
    global animation_interrupt

    print("enqueue_animation")
    print("current status", animation_running, animation_priority)
    if animation_running and priority < animation_priority:
        print("enqueue_animation interrupt")
        animation_interrupt = True

    heapq.heappush(animation_heap, AnimationItem(priority, animation))

    if not animation_running:
        print("enqueue_animation create task")
        animation_running = True
        animation_priority = priority
        asyncio.create_task(run_animations())

class AnimationInterrupt(Exception):
    pass

class AnimationClear(Exception):
    pass

async def stop_animation():
    raise AnimationClear()
    
async def run_animations():
    global animation_heap
    global animation_priority
    global animation_running
    global animation_interrupt

    print("run_animations start")
    try:
        while len(animation_heap):
            print("run_animations next")

            animation_item = heapq.heappop(animation_heap)

            animation_priority = animation_item.priority
            animation = animation_item.animation

            try:
                await animation
            except AnimationInterrupt:
                print("run_animations interrupt")
            except AnimationClear:
                animation_heap.clear()
            animation_interrupt = False
    finally:
        animation_priority = None
        animation_running = False
        animation_interrupt = False
    print("run_animations finish")

@server.route("/", methods=["GET"])
def index(request):
    return send_file("index.html")

ENABLE_TUNE = True

@server.route("/tune", methods=["GET"])
async def tune(request):
    if ENABLE_TUNE:
        if request.args['type'] == 'int':
            value = int(request.args['value'])
        if request.args['type'] == 'float':
            value = float(request.args['value'])
        if request.args['type'] == 'str':
            value = str(request.args['value'])
        globals()[request.args['var']] = value
    return 'set'

@server.route("/reset", methods=["GET"])
def reset(request):
    asyncio.create_task(reset_after_delay())
    return 'reset'

async def reset_after_delay():
    print('resetting')
    await asyncio.sleep(0.5)
    machine.reset()

@server.route("/stop", methods=["GET"])
def stop(request):
    enqueue_animation(stop_animation(), priority=-1)
    return 'stopped'

@server.route("/countdown", methods=["GET"])
async def countdown(request):
    seconds = int(request.args['seconds'])
    timestamp = time.time() + seconds
    enqueue_animation(countdown_animation(timestamp), priority=2)
    return 'started'

@server.route("/test", methods=["GET"])
async def test(request):
    enqueue_animation(
        test_animation(),
        priority=1
    )
    return 'test'

message_inbox = []

@server.route("/message", methods=["POST"])
async def message(request):
    global message_inbox

    enqueue_animation(message_animation(request.json), priority=1)
    return 'message'

@server.route("/clear-inbox", methods=["GET"])
async def clear_inbox(request):
    global message_inbox

    message_inbox.clear()
    enqueue_animation(inbox_animation(), priority=2)
    return 'cleared'

@server.route("/read-inbox", methods=["GET"])
async def read_inbox(request):
    global message_inbox

    for message in message_inbox:
        message['read'] = True
        enqueue_animation(message_animation(message), priority=1)
    message_inbox.clear()
    enqueue_animation(inbox_animation(), priority=3)
    return 'read'

@server.route("/start-clock", methods=["GET"])
async def start_clock(request):
    start_timestamp = int(request.args['start_timestamp'])
    enqueue_animation(clock_animation(start_timestamp), priority=5)
    return 'started'

@server.route("/spacex", methods=["GET"])
async def spacex(request):
    enqueue_animation(spacex_animation(), priority=2)
    return 'started'

async def test_animation():
    graphics.set_font(font)

    sleep_reset()

    counter = 20

    vertical_offset = -2
    padding = 2

    while True:
        clock = "{:02}".format(counter)
        graphics.set_pen(BLACK)
        graphics.clear()
        graphics.set_pen(WHITE)
        graphics.text(clock, 0, vertical_offset, scale=1, spacing=2)
        unicorn.update(graphics)

        for i in range(20):
            await sleep_frame()

        previous_clock = clock
        counter -= 1

        if counter < 0:
            break

        new_clock = "{:02}".format(counter)

        for i in range(len(previous_clock)):
            if previous_clock[i] != new_clock[i]:
                movement_index = i
                break

        y = vertical_offset

        while True:
            await sleep_frame()

            y += 1

            if y > 11 + vertical_offset + padding:
                break

            graphics.set_pen(BLACK)
            graphics.clear()
            graphics.set_pen(WHITE)

            fixed_clock = new_clock[:movement_index]
            graphics.text(fixed_clock, 0, vertical_offset, scale=1, spacing=2)

            x = graphics.measure_text(fixed_clock, scale=1, spacing=2)

            graphics.text(new_clock[movement_index:], x, y-11-padding, scale=1, spacing=2)
            graphics.text(previous_clock[movement_index:], x, y, scale=1, spacing=2)
            unicorn.update(graphics)

    enqueue_animation(inbox_animation(), priority=3)

CLOCK_RAINBOW_HOLD_TIME = 3

async def clock_animation(start_timestamp):
    graphics.set_font(font)
    sleep_reset()

    start_time = time.time()

    clock = None
    y = -2 - 11

    frame = 0
    while True:
        await sleep_frame()
        frame += 1

        if y < -2:
            y += 1

        diff = time.time() - start_time

        timestamp = start_timestamp + diff

        hours = (timestamp // 3600) % 12
        minutes = (timestamp // 60) % 60
        seconds = timestamp % 60

        previous_clock = clock
        clock = "{:02}:{:02}".format(hours, minutes)

        w = graphics.measure_text(clock, scale=1, spacing=2)
        x = int(10 + (WIDTH-10) / 2 - w / 2 + 1)

        if previous_clock is None or clock == previous_clock or y != -2:
            graphics.set_pen(BLACK)
            graphics.clear()
            graphics.set_pen(WHITE)

            draw_icon(graphics, CLOCK, 0, y)
            graphics.text(clock, x, y, scale=1, spacing=2)

            if minutes == 0 and seconds < CLOCK_RAINBOW_HOLD_TIME:
                apply_rainbow(frame)

            unicorn.update(graphics)

        else:
            ranges = []

            start = None
            kind = None
            for i in range(len(previous_clock)):
                if kind is None:
                    if previous_clock[i] == clock[i]:
                        kind = 'fixed'
                        start = i
                    else:
                        kind = 'moving'
                        start = i
                elif kind == 'fixed':
                    if previous_clock[i] != clock[i]:
                        ranges.append((kind, start, i))
                        kind = 'moving'
                        start = i
                elif kind == 'moving':
                    if previous_clock[i] == clock[i]:
                        ranges.append((kind, start, i))
                        kind = 'fixed'
                        start = i
            ranges.append((kind, start, len(previous_clock)))
            
            movement_y = 0
            padding = 2
            while True:
                await sleep_frame()
                frame += 1

                movement_y += 1

                if movement_y > 11 + padding:
                    break

                graphics.set_pen(BLACK)
                graphics.clear()
                graphics.set_pen(WHITE)

                draw_icon(graphics, CLOCK, 0, y)

                movement_x = 0

                for kind, start, end in ranges:
                    if kind == 'fixed':
                        text = clock[start:end]
                        graphics.text(text, x + movement_x, y, scale=1, spacing=2)
                        movement_x += graphics.measure_text(text, scale=1, spacing=2)
                    elif kind == 'moving':
                        text = clock[start:end]
                        previous_text = previous_clock[start:end]
                        graphics.text(text, x + movement_x, y + movement_y - 11 - padding, scale=1, spacing=2)
                        graphics.text(previous_text, x + movement_x, y + movement_y, scale=1, spacing=2)
                        movement_x += graphics.measure_text(text, scale=1, spacing=2)

                if minutes == 0:
                    apply_rainbow(frame)

                unicorn.update(graphics)

async def countdown_animation(timestamp):
    graphics.set_font(font)

    sleep_reset()

    diff_seconds = min(3599, max(0, timestamp - time.time()))

    target = time.ticks_add(time.ticks_ms(), diff_seconds * 1000)

    y = -2 - 11

    clock = None

    try:
        frame = 0
        while True:
            await sleep_frame()
            frame += 1

            if y < -2:
                y += 1

            diff = time.ticks_diff(target, time.ticks_ms()) / 1000.0

            if diff < -5:
                if y < 9:
                    y += 1
                else:
                    break

            timer = max(0, diff)
            minute = math.floor(timer / 60 % 60)
            second = math.floor(timer % 60)

            previous_clock = clock
            clock = "{:02}:{:02}".format(minute, second)

            w = graphics.measure_text(clock, scale=1, spacing=2)
            x = int(10 + (WIDTH-10) / 2 - w / 2 + 1)

            if previous_clock is None or clock == previous_clock or y != -2:
                graphics.set_pen(BLACK)
                graphics.clear()
                graphics.set_pen(WHITE)

                draw_icon(graphics, CALENDAR, 0, y)
                graphics.text(clock, x, y, scale=1, spacing=2)

                if timer < RAINBOW_TIMER_THRESHOLD:
                    apply_rainbow(frame)

                unicorn.update(graphics)
            else:
                ranges = []

                start = None
                kind = None
                for i in range(len(previous_clock)):
                    if kind is None:
                        if previous_clock[i] == clock[i]:
                            kind = 'fixed'
                            start = i
                        else:
                            kind = 'moving'
                            start = i
                    elif kind == 'fixed':
                        if previous_clock[i] != clock[i]:
                            ranges.append((kind, start, i))
                            kind = 'moving'
                            start = i
                    elif kind == 'moving':
                        if previous_clock[i] == clock[i]:
                            ranges.append((kind, start, i))
                            kind = 'fixed'
                            start = i
                ranges.append((kind, start, len(previous_clock)))
                
                movement_y = 0
                padding = 2
                while True:
                    await sleep_frame()
                    frame += 1

                    movement_y += 1

                    if movement_y > 11 + padding:
                        break

                    graphics.set_pen(BLACK)
                    graphics.clear()
                    graphics.set_pen(WHITE)

                    draw_icon(graphics, CALENDAR, 0, y)

                    movement_x = 0

                    for kind, start, end in ranges:
                        if kind == 'fixed':
                            text = clock[start:end]
                            graphics.text(text, x + movement_x, y, scale=1, spacing=2)
                            movement_x += graphics.measure_text(text, scale=1, spacing=2)
                        elif kind == 'moving':
                            text = clock[start:end]
                            previous_text = previous_clock[start:end]
                            graphics.text(text, x + movement_x, y + movement_y - 11 - padding, scale=1, spacing=2)
                            graphics.text(previous_text, x + movement_x, y + movement_y, scale=1, spacing=2)
                            movement_x += graphics.measure_text(text, scale=1, spacing=2)

                    if timer < RAINBOW_TIMER_THRESHOLD:
                        apply_rainbow(frame)

                    unicorn.update(graphics)

        enqueue_animation(inbox_animation(), priority=3)
    except AnimationInterrupt:
        enqueue_animation(countdown_animation(timestamp), priority=2)
        raise

async def spacex_animation():
    graphics.set_font(font)

    graphics.set_pen(YELLOW)
    graphics.pixel(0, 0)
    unicorn.update(graphics)

    while True:
        try:
            ntptime.settime()
            break
        except:
            time.sleep(1)
            pass

    graphics.set_pen(BLUE)
    graphics.pixel(0, 0)
    unicorn.update(graphics)

    response = requests.get("https://sxcontent9668.azureedge.us/cms-assets/future_missions.json")
    content = json.loads(deflate.DeflateIO(io.StringIO(response.content)).read())

    mission_id = min(content, key=lambda mission_id: content[mission_id]['Order'])

    graphics.set_pen(GREEN)
    graphics.pixel(0, 0)
    unicorn.update(graphics)

    sleep_reset()

    y = -2 - 11

    paused = False
    clock = None
    last_update = None
    frame = 0

    while True:
        await sleep_frame()
        frame += 1

        if y < -2:
            y += 1

        if last_update is None or time.time() - last_update > (10 if paused else 30):
            try:
                response = requests.get("https://sxcontent9668.azureedge.us/cms-assets/future_missions.json")
                content = json.loads(deflate.DeflateIO(io.StringIO(response.content)).read())

                tzero = content[mission_id]['TZeroLaunchDate']['Seconds']
                paused = content[mission_id]['TZeroPaused']
            except:
                pass

            last_update = time.time()
            sleep_reset()

        timer = abs(time.time() - tzero)

        hour = math.floor(timer / 3600)
        minute = math.floor(timer / 60 % 60)
        second = math.floor(timer % 60)

        previous_clock = clock

        if paused:
            clock = "HOLD"
        else:
            clock = "{}:{:02}:{:02}".format(hour, minute, second)

        w = graphics.measure_text(clock, scale=1, spacing=2)
        x = int(WIDTH / 2 - w / 2 + 1)

        if previous_clock is None or clock == previous_clock or y != -2:
            graphics.set_pen(BLACK)
            graphics.clear()
            graphics.set_pen(WHITE)

            graphics.text(clock, x, y, scale=1, spacing=2)

            if timer < 10:
                apply_rainbow(frame)

            unicorn.update(graphics)
        elif len(previous_clock) != len(clock):
            movement_y = 0
            padding = 2
            while True:
                await sleep_frame()
                frame += 1

                movement_y += 1

                if movement_y > 11 + padding:
                    break

                graphics.set_pen(BLACK)
                graphics.clear()
                graphics.set_pen(WHITE)

                previous_w = graphics.measure_text(previous_clock, scale=1, spacing=2)
                previous_x = int(WIDTH / 2 - previous_w / 2 + 1)

                graphics.text(clock, x, y + movement_y - 11 - padding, scale=1, spacing=2)
                graphics.text(previous_clock, previous_x, y + movement_y, scale=1, spacing=2)

                if timer < 10:
                    apply_rainbow(frame)

                unicorn.update(graphics)
        else:
            ranges = []

            start = None
            kind = None
            for i in range(len(previous_clock)):
                if kind is None:
                    if previous_clock[i] == clock[i]:
                        kind = 'fixed'
                        start = i
                    else:
                        kind = 'moving'
                        start = i
                elif kind == 'fixed':
                    if previous_clock[i] != clock[i]:
                        ranges.append((kind, start, i))
                        kind = 'moving'
                        start = i
                elif kind == 'moving':
                    if previous_clock[i] == clock[i]:
                        ranges.append((kind, start, i))
                        kind = 'fixed'
                        start = i
            ranges.append((kind, start, len(previous_clock)))
            
            movement_y = 0
            padding = 2
            while True:
                await sleep_frame()
                frame += 1

                movement_y += 1

                if movement_y > 11 + padding:
                    break

                graphics.set_pen(BLACK)
                graphics.clear()
                graphics.set_pen(WHITE)

                movement_x = 0

                for kind, start, end in ranges:
                    if kind == 'fixed':
                        text = clock[start:end]
                        graphics.text(text, x + movement_x, y, scale=1, spacing=2)
                        movement_x += graphics.measure_text(text, scale=1, spacing=2)
                    elif kind == 'moving':
                        text = clock[start:end]
                        previous_text = previous_clock[start:end]
                        graphics.text(text, x + movement_x, y + movement_y - 11 - padding, scale=1, spacing=2)
                        graphics.text(previous_text, x + movement_x, y + movement_y, scale=1, spacing=2)
                        movement_x += graphics.measure_text(text, scale=1, spacing=2)

                if timer < 10:
                    apply_rainbow(frame)

                unicorn.update(graphics)


rainbow_pens = []
rainbow_pens_len = WIDTH * 2
for i in range(0, rainbow_pens_len):
    rainbow_pens.append(
        graphics.create_pen_hsv(
            (i/rainbow_pens_len),
            1.0,
            1.0
        )
    )

@micropython.native # pyright: ignore
def apply_rainbow(frame):
    for i in range(0, WIDTH):
        for j in range(0, HEIGHT):
            if graphics_view[(i + j * WIDTH) * 4] == 255:
                pen_index = (i + j + frame * RAINBOW_FRAME_MULTIPLIER) % rainbow_pens_len
                pen = rainbow_pens[pen_index]
                graphics.set_pen(pen)
                graphics.pixel(i, j)

# https://github.com/halfmage/pixelarticons/
CALENDAR = bytearray([
    0b00001111,0b11111000,
    0b00001000,0b00101000,
    0b00001000,0b10101100,
    0b00001000,0b00101000,
    0b00001000,0b10101000,
    0b00001000,0b00101000,
    0b00001000,0b10101100,
    0b00001000,0b00101000,
    0b00001111,0b11111000,
    0b00000000,0b00000000, # Calendar
])
CLOCK = bytearray([
    0b00000111,0b11110000,
    0b00001000,0b00001000,
    0b00001000,0b00001000,
    0b00001000,0b00001000,
    0b00001001,0b11101000,
    0b00001001,0b00001000,
    0b00001001,0b00001000,
    0b00001000,0b00001000,
    0b00000111,0b11110000,
    0b00000000,0b00000000, # Clock
])

@micropython.native # pyright: ignore
def draw_icon(graphics, icon, x, y):
    for row in range(0, 10):
        for column in range(0, 16):
            byte_index = row * 2 + (column // 8)
            bit_index = 7 - (column % 8)
            
            if icon[byte_index] & (1 << bit_index):
                graphics.pixel(x + row, y + (15 - column))

ALL_CHARACTERS = ""
for i in range(96):
    ALL_CHARACTERS += chr(ord(' ') + i)

DIRECTIONS = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0)
]

async def inbox_animation():
    global message_inbox

    snake = []

    state = None
    for _ in range(len(message_inbox) + (1 if len(message_inbox) else 0)):
        if state is None:
            state = (
                random.randint(0, WIDTH - 1),
                random.randint(0, HEIGHT - 1),
                random.randint(0, 3)
            )
        else:
            if random.randint(0, 100) < 10:
                rotate = random.randint(-1, 1)
            else:
                rotate = 0

            direction = (state[2] + rotate + 4) % 4

            state = (
                (state[0] + DIRECTIONS[direction][0] + WIDTH) % WIDTH,
                (state[1] + DIRECTIONS[direction][1] + HEIGHT) % HEIGHT,
                direction
            )

        snake.insert(0, state)

    frame = 0
    remainder = 0
    while True:
        await sleep_frame()
        remainder += 1

        if remainder == 2:
            frame += 1

        if remainder == 9 and len(snake):
            remainder = 0
            head = snake[0]

            if random.randint(0, 100) < 10:
                rotate = random.randint(-1, 1)
            else:
                rotate = 0

            direction = (head[2] + rotate + 4) % 4
            
            snake.insert(0, (
                (head[0] + DIRECTIONS[direction][0] + WIDTH) % WIDTH,
                (head[1] + DIRECTIONS[direction][1] + HEIGHT) % HEIGHT,
                direction
            ))
            snake.pop()

        graphics.set_pen(BLACK)
        graphics.clear()
        graphics.set_pen(WHITE)
        if len(snake):
            head = snake[0]
            graphics.pixel(head[0], head[1])
            apply_rainbow(frame)
            graphics.set_pen(WHITE)
            for x, y, direction in snake[1:]:
                graphics.pixel(x, y)
        unicorn.update(graphics)

        if len(snake) == 0:
            break

async def message_animation(message):
    global message_inbox

    text = message['text']
    read = message['read']

    enable_rainbow = 'rainbow' in message['effects']

    graphics.set_font(font)

    sleep_reset()

    w = graphics.measure_text(text, scale=1, spacing=2)

    x = 0
    y = 11

    frame = 0
    remainder = 0
    while y > -2:
        await sleep_frame()

        frame += 2
        remainder += 1

        if remainder == 2:
            remainder = 0
            y -= 1

        graphics.set_pen(BLACK)
        graphics.clear()
        graphics.set_pen(WHITE)
        graphics.text(text, x, y, scale=1, spacing=2)

        if enable_rainbow:
            apply_rainbow(frame)

        unicorn.update(graphics)

    if not read:
        message_inbox.append(message)

    if HOLD_ENABLE:
        hold = HOLD_TIME
        while hold > 0:
            await sleep_frame()

            frame += 1
            hold -= 1

            graphics.set_pen(BLACK)
            graphics.clear()
            graphics.set_pen(WHITE)
            graphics.text(text, x, y, scale=1, spacing=2)

            if enable_rainbow:
                apply_rainbow(frame)

            unicorn.update(graphics)

    while x > -w:
        await sleep_frame()

        frame += 2
        x -= 1

        graphics.set_pen(BLACK)
        graphics.clear()
        graphics.set_pen(WHITE)
        graphics.text(text, x, y, scale=1, spacing=2)

        if enable_rainbow:
            apply_rainbow(frame)

        unicorn.update(graphics)

    enqueue_animation(inbox_animation(), priority=3)

frame_start = None

LATE_FRAME_ENABLE = True

async def sleep_frame():
    global frame_start
    global animation_interrupt

    if animation_interrupt:
        raise AnimationInterrupt()

    frame_end = time.ticks_ms()

    if frame_start is not None:
        sleep_time = max(0, DELAY_MILLIS - time.ticks_diff(frame_end, frame_start))

        if LATE_FRAME_ENABLE:
            if sleep_time < 1:
                graphics.set_pen(RED)
                graphics.line(0, 0, WIDTH - 1, 0)
                graphics.line(0, 0, 0, HEIGHT - 1)
                graphics.line(WIDTH - 1, 0, WIDTH - 1, HEIGHT - 1)
                graphics.line(0, HEIGHT - 1, WIDTH - 1, HEIGHT - 1)
                unicorn.update(graphics)

        await asyncio.sleep_ms(sleep_time)

    frame_start = time.ticks_ms()

def sleep_reset():
    global frame_start
    frame_start = None


async def main():
    asyncio.create_task(server.start_server(host="0.0.0.0", port=80, debug=True))

    graphics.set_pen(GREEN)
    graphics.pixel(0, 0)
    unicorn.update(graphics)

    enqueue_animation(inbox_animation(), priority=3)
    while True:
        await asyncio.sleep(1)

asyncio.run(main())
