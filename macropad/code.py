from adafruit_macropad import MacroPad
import json
import supervisor
import time
import usb_cdc
import displayio
import adafruit_display_text.bitmap_label
import adafruit_display_text.label
import adafruit_display_text as display_text
import terminalio
import adafruit_fancyled.adafruit_fancyled as fancy
import vectorio
import adafruit_pcf8591.pcf8591 as PCF
from adafruit_pcf8591.analog_in import AnalogIn
from rainbowio import colorwheel
from adafruit_seesaw.seesaw import Seesaw
from adafruit_seesaw.analoginput import AnalogInput
from adafruit_seesaw import neopixel
from adafruit_dps310.basic import DPS310
import adafruit_vl6180x
import board

i2c = board.I2C()

neoslider = Seesaw(i2c, 0x30)
potentiometer = AnalogInput(neoslider, 18)

pcf = PCF.PCF8591(i2c)
pcf_in_0 = AnalogIn(pcf, PCF.A0)
pcf_in_3 = AnalogIn(pcf, PCF.A3)

macropad = MacroPad()

macropad.pixels[0] = fancy.CRGB(1.0, 1.0, 0.0).pack()
macropad.pixels.show()

# distance_sensor = adafruit_vl6180x.VL6180X(i2c)

# distance_sensor.start_range_continuous(2000)
# distance_sensor.start_range_continuous(250)

# last_activity = time.time()
# while True:
    # value = distance_sensor.range_from_history
    # last_activity_diff = time.time() - last_activity

# while True:
    # value = distance_sensor.read_lux(adafruit_vl6180x.ALS_GAIN_1)
    # print((value,))
    # time.sleep(0.06)


# dps310 = DPS310(i2c)

# pixels = neopixel.NeoPixel(neoslider, 14, 4, pixel_order=neopixel.GRB)

# def potentiometer_to_color(value):
    # """Scale the potentiometer values (0-1023) to the colorwheel values (0-255)."""
    # return value / 1023 * 255

# while True:
    # print("%.2f *C" % dps310.temperature)
    # print("%.2f hPa" % dps310.pressure)
    # print("")
    # Fill the pixels a color based on the position of the potentiometer.
    # pixels.fill(colorwheel(potentiometer_to_color(potentiometer.value)))
    # time.sleep(0.1)

def display_sleep():
    macropad.display.bus.send(int(0xAE), '')

def display_wake():
    macropad.display.bus.send(int(0xAF), '')

def clear_pixels():
    macropad.pixels.fill(0x000000)
    macropad.pixels.show()

def send_message(message):
    message_json = json.dumps(message)
    print('->', message_json)
    usb_cdc.data.write(bytes(message_json + '\n', 'utf-8'))
    usb_cdc.data.flush()

def send_potentiometer(value):
    print('->', 'potentiometer')
    usb_cdc.data.write(bytes('p' + str(value) + '\n', 'utf-8'))
    usb_cdc.data.flush()

def send_x_axis(value):
    print('->', 'x axis')
    usb_cdc.data.write(bytes('x' + str(value) + '\n', 'utf-8'))
    usb_cdc.data.flush()

def send_y_axis(value):
    print('->', 'y axis' + str(value))
    usb_cdc.data.write(bytes('y' + str(value) + '\n', 'utf-8'))
    usb_cdc.data.flush()

message_buffer = ''
usb_cdc.data.timeout = 0

def get_message():
    global message_buffer

    message_buffer += usb_cdc.data.read(100).decode('utf-8')

    index = message_buffer.find('\n')

    if index == -1:
        return None

    message_json = message_buffer[:index]
    message_buffer = message_buffer[index+1:]

    try:
        message = json.loads(message_json)
    except:
        print('<- (error)', bytes(message_json, 'utf-8'))
        return None

    in_waiting = usb_cdc.data.in_waiting
    print(f'<- {in_waiting:3}', json.dumps(message))

    if message['kind'] == 'heartbeat':
        return None

    return message

last_activity = time.time()

def reset_activity_timer():
    global last_activity
    last_activity = time.time()

def check_activity_timeout(next_name):
    global last_activity
    global state

    if time.time() - last_activity < 3:
        return False

    state['name'] = 'sleep'
    state['sleep_next_name'] = next_name
    return True

key_event_buffer = []
key_state = [False for _ in range(12)]

def get_key_event(peek=False):
    global key_event_buffer

    if len(key_event_buffer):
        key_event = key_event_buffer.pop(0)
    else:
        key_event = macropad.keys.events.get()

    if key_event is not None:
        reset_activity_timer()

        key_state[key_event.key_number] = key_event.pressed

        if peek:
            key_event_buffer.append(key_event)

    return key_event

last_encoder = macropad.encoder

def get_encoder_diff(peek=False):
    global last_encoder
    encoder = macropad.encoder
    diff = encoder - last_encoder

    if diff != 0:
        reset_activity_timer()

    if not peek:
        last_encoder = encoder

    return diff

last_potentiometer = potentiometer.value

def get_potentiometer_diff(peek=False):
    global last_potentiometer
    value = potentiometer.value
    diff = value - last_potentiometer

    if abs(diff) > 10:
        reset_activity_timer()

    if not peek:
        last_potentiometer = value

    return diff

def get_x_axis_value(raw_value):
    return 32768 - raw_value

def get_y_axis_value(raw_value):
    return 32768 - raw_value

last_x_axis = get_x_axis_value(pcf_in_0.value)
last_y_axis = get_y_axis_value(pcf_in_3.value)

def get_x_axis_diff(peek=False):
    global last_x_axis

    value = get_x_axis_value(pcf_in_0.value)

    if abs(value) < 4000:
        value = 0

    diff = value - last_x_axis

    if diff != 0:
        reset_activity_timer()

    if not peek:
        last_x_axis = value

    return diff

def get_y_axis_diff(peek=False):
    global last_y_axis

    value = get_y_axis_value(pcf_in_3.value)

    if abs(value) < 4000:
        value = 0

    diff = value - last_y_axis

    if diff != 0:
        reset_activity_timer()

    if not peek:
        last_y_axis = value

    return diff


distance_delay = None

def set_distance_delay(delay):
    return

    global distance_delay

    if delay == distance_delay:
        return

    print('=', 'distance delay', delay)
    distance_sensor.stop_range_continuous()
    if delay is not None and delay <= 2000:
        time.sleep(0.3)
        distance_sensor.start_range_continuous(delay)
    distance_delay = delay

set_distance_delay(250)

last_range = time.time()

def get_distance():
    return 255

    global last_range

    if distance_delay is None:
        value = 255
    elif distance_delay <= 2000:
        value = distance_sensor.range_from_history
    else:
        if time.time() - last_range > distance_delay/1000:
            value = distance_sensor.range
            last_range = time.time()
        else:
            value = distance_sensor.range_from_history

    if value < 255:
        reset_activity_timer()

    return value

last_light_time = time.time()
last_light = 255 # distance_sensor.read_lux(adafruit_vl6180x.ALS_GAIN_1)

def get_light_diff():
    return 0

    global last_light
    global last_light_time

    if time.time() - last_light_time < 1:
        return 0

    value = distance_sensor.read_lux(adafruit_vl6180x.ALS_GAIN_1)
    diff = value - last_light
    last_light = value
    last_light_time = time.time()

    return diff

def create_palette(foreground, background):
    palette = displayio.Palette(2)
    palette[0] = background.pack()
    palette[1] = foreground.pack()
    return palette;

def gamma_adjust(value):
    return fancy.gamma_adjust(value, gamma_value=2.7, brightness=0.3)

linear_colors = dict(
    black=fancy.CHSV(0.0, 0.0, 0.0),
    white=fancy.CHSV(0.0, 0.0, 1.0),
    green=fancy.CRGB(0.0, 1.0, 0.0),
    red=fancy.CRGB(1.0, 0.0, 0.0),
    yellow=fancy.CRGB(1.0, 1.0, 0.0),
    magenta=fancy.CRGB(1.0, 0.0, 1.0),
    light_blue=fancy.unpack(0x89CFF0),

    cyan=fancy.unpack(0x00CED1),
    light_purple=fancy.unpack(0xE6E6FA),
    peach=fancy.unpack(0xFFDAB9),
    #navy_blue=fancy.unpack(0x001F3F),
    forest_green=fancy.unpack(0x229B22),
    burgundy=fancy.unpack(0x800020),
    cream=fancy.unpack(0xFFFDD0),
    gray=fancy.unpack(0xA9A9A9),
    amber=fancy.unpack(0xFFC107),
)

palettes = dict(
    normal=create_palette(
        foreground=linear_colors['white'],
        background=linear_colors['black'],
    ),
    inverted=create_palette(
        foreground=linear_colors['black'],
        background=linear_colors['white'],
    ),
)

linear_colors_50 = {
    k: fancy.mix(v, linear_colors['black'], 0.5)
    for k, v in linear_colors.items()
}

colors = { k: gamma_adjust(v) for k, v in linear_colors.items() }
colors_50 = { k: gamma_adjust(v) for k, v in linear_colors_50.items() }

gradients_50 = {
    k: gamma_adjust(
        fancy.expand_gradient(
            [
                (0.0, linear_colors['black']),
                (1.0, v)
            ],
            25,
        )
    )
    for k, v in linear_colors_50.items()
}

def wait_for_reply_animated(index, gradient, timeout=8000):
    clear_pixels()
    start = supervisor.ticks_ms()
    while True:
        position = supervisor.ticks_ms() - start

        color = fancy.palette_lookup(gradient, position / 1000)
        macropad.pixels[index] = color.pack()
        macropad.pixels.show()

        message = get_message()

        if message is not None:
            return message

        if position > timeout:
            return None

def check_response(message, pixel_index, success_color):
    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[pixel_index] = success_color.pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

def show_error():
    macropad.pixels.fill(colors['magenta'].pack())
    macropad.pixels.show()
    time.sleep(0.3)
    clear_pixels()

def set_option_tile_grid_selected(tile_grid, selected):
    palette = palettes['inverted' if selected else 'normal']
    tile_grid.pixel_shader = palette

def set_toolbar_pixels():
    macropad.pixels[9] = colors_50['white'].pack()
    macropad.pixels[10] = colors_50['white'].pack()
    macropad.pixels[11] = colors_50['cyan'].pack()

def startup():
    global state

    clear_pixels()
    macropad.pixels[0] = colors_50['green'].pack()
    macropad.pixels.show()

    state['name'] = 'toggl'

def sleep():
    global state

    while macropad.pixels.brightness > 0:
        macropad.pixels.brightness = max(0, macropad.pixels.brightness - 0.05)
        macropad.pixels.show()

        macropad.display.brightness = max(0, macropad.display.brightness - 0.05)

        time.sleep(0.02)

    display_sleep();

    while True:
        get_message()
        key_event = get_key_event(peek=True)

        if key_event:
            break

    reset_activity_timer()
    display_wake()

    while macropad.pixels.brightness < 1:
        macropad.pixels.brightness = min(1, macropad.pixels.brightness + 0.05)
        macropad.pixels.show()

        macropad.display.brightness = min(1, macropad.display.brightness + 0.5)

        time.sleep(0.01)

    macropad.display.brightness = 1

    state['name'] = state['sleep_next_name']

def apps():
    global state

    group = displayio.Group(y=macropad.display.height//2)

    label = display_text.bitmap_label.Label(
        font=terminalio.FONT,
        text=state['apps_source_name']
    )

    group.append(label)
    macropad.display.show(group)
    macropad.display.refresh()

    clear_pixels()
    macropad.pixels[0] = colors_50['light_purple'].pack()
    macropad.pixels[1] = colors_50['cream'].pack()
    macropad.pixels[2] = colors_50['light_blue'].pack()
    macropad.pixels[3] = colors_50['yellow'].pack()

    set_toolbar_pixels()
    macropad.pixels.show()

    while True:
        get_message()
        key_event = get_key_event()

        if key_event:
            if key_event.key_number == 11 and not key_event.pressed:
                state['name'] = label.text
                break

            if key_event.pressed:
                if key_event.key_number == 0:
                    label.text = 'toggl'

                if key_event.key_number == 1:
                    label.text = 'unicorn'

                if key_event.key_number == 2:
                    label.text = 'bluetooth'

                if key_event.key_number == 3:
                    label.text = 'servo'

                macropad.display.refresh()

def toggl():
    global state

    if not state['toggl_options_loaded']:
        state['name'] = 'toggl_get_time_entries'
        return

    clear_pixels()
    macropad.pixels[0] = colors_50['yellow'].pack()
    macropad.pixels[1] = colors_50['red'].pack()
    macropad.pixels[2] = colors_50['green'].pack()
    macropad.pixels[3] = colors_50['light_blue'].pack()

    macropad.pixels[5] = colors_50['white'].pack()
    macropad.pixels[8] = colors_50['white'].pack()

    set_toolbar_pixels()
    macropad.pixels.show()

    macropad.display.show(state['toggl_options_group'])
    macropad.display.refresh()

    while True:
        if check_activity_timeout('toggl'):
            break

        get_message()

        key_event = get_key_event()

        previous_index = state['toggl_index']

        if key_event and key_event.pressed:
            if key_state[11]:
                state['name'] = 'apps'
                state['apps_source_name'] = 'toggl'
                break

            if key_event.key_number == 0:
                state['name'] = 'toggl_get_time_entries'
                break

            if key_event.key_number == 1:
                state['name'] = 'toggl_send_stop'
                break

            if key_event.key_number == 2:
                if key_state[9]:
                    state['name'] = 'toggl_send_continue'
                    break
                else:
                    state['name'] = 'toggl_send_start'
                    break

            if key_event.key_number == 3:
                state['name'] = 'toggl_adjust_time'
                break

            if key_state[10]:
                if key_event.key_number == 5:
                    state['toggl_index'] -= 4 + (state['toggl_index'] % 4)

                if key_event.key_number == 8:
                    state['toggl_index'] += 4 - (state['toggl_index'] % 4)
            elif key_state[9]:
                if key_event.key_number == 5:
                    state['toggl_index'] = 0

                if key_event.key_number == 8:
                    state['toggl_index'] = len(state['toggl_options']) - 1
            else:
                if key_event.key_number == 5:
                    state['toggl_index'] -= 1

                if key_event.key_number == 8:
                    state['toggl_index'] += 1

            state['toggl_index'] = (
                state['toggl_index'] + len(state['toggl_options'])
            ) % len(state['toggl_options'])

        if state['toggl_index'] != previous_index:
            set_option_tile_grid_selected(
                state['toggl_options_group'][previous_index],
                False
            )

            set_option_tile_grid_selected(
                state['toggl_options_group'][state['toggl_index']],
                True
            )

            page = int(state['toggl_index'] / 4)
            target_options_group_y = -page*64
            display_options_group_y = state['toggl_options_group'].y

            macropad.display.refresh()

            while state['toggl_options_group'].y != target_options_group_y:
                display_options_group_y += (
                    (target_options_group_y - display_options_group_y) * 0.5
                )
                state['toggl_options_group'].y = round(display_options_group_y)
                macropad.display.refresh()

def toggl_get_time_entries():
    global state

    state['toggl_options_loaded'] = True

    send_message(dict(kind='getTimeEntries'))

    entries = []

    clear_pixels()
    start = supervisor.ticks_ms()

    while True:
        position = supervisor.ticks_ms() - start

        color = fancy.palette_lookup(gradients_50['yellow'], position / 1000)
        macropad.pixels[0] = color.pack()
        macropad.pixels.show()

        message = get_message()

        if message:
            if message['kind'] == 'timeEntry':
                entries.append(message['timeEntry']['description'])
            else:
                break

        if position > 5000:
            break

    if not message or message['kind'] != 'success':
        show_error()
        reset_activity_timer()
        state['name'] = 'toggl'
        return

    options = list(set(entries))
    options.sort()

    if not len(options):
        options.append('')

    clear_pixels()
    reset_activity_timer()

    state['toggl_index'] = 0
    state['toggl_options'] = options

    state['toggl_options_group'] = displayio.Group()

    for i, option in enumerate(options):
        label = display_text.bitmap_label.Label(
            font=terminalio.FONT,
            text=option
        )

        palette = palettes['normal']

        label_tile_grid = displayio.TileGrid(
            bitmap=label.bitmap,
            pixel_shader=palette,
            y=i*16,
        )

        state['toggl_options_group'].append(label_tile_grid)

        selected = i == state['toggl_index']
        set_option_tile_grid_selected(label_tile_grid, selected)

    state['name'] = 'toggl'

def toggl_send_start():
    global state

    send_message(dict(
        kind='startTimeEntry',
        timeEntry=dict(
            description=state['toggl_options'][state['toggl_index']],
        ),
    ))

    message = wait_for_reply_animated(2, gradients_50['green'])
    check_response(message, 2, colors_50['green'])

    reset_activity_timer()
    state['name'] = 'toggl'

def toggl_send_continue():
    global state

    send_message(dict(
        kind='continueTimeEntry',
    ))

    message = wait_for_reply_animated(2, gradients_50['green'])
    check_response(message, 2, colors_50['green'])

    reset_activity_timer()
    state['name'] = 'toggl'

def toggl_send_stop():
    global state

    send_message(dict(kind='stopTimeEntry'))

    message = wait_for_reply_animated(1, gradients_50['red'])
    check_response(message, 1, colors_50['red'])

    reset_activity_timer()
    state['name'] = 'toggl'

def toggl_adjust_time():
    global state

    clear_pixels()
    macropad.pixels[1] = colors_50['red'].pack()
    macropad.pixels[2] = colors_50['green'].pack()
    macropad.pixels[5] = colors_50['white'].pack()
    macropad.pixels[8] = colors_50['white'].pack()
    macropad.pixels[9] = colors_50['white'].pack()
    macropad.pixels.show()

    group = displayio.Group(
        x=macropad.display.width//2,
        y=macropad.display.height//2,
    )

    group.append(vectorio.Polygon(
        pixel_shader=palettes['inverted'],
        points=[
            (0, 0),
            (-4, -4),
            (4, -4),
        ],
        x=0,
        y=-5,
    ))

    scale_group = displayio.Group()

    scale_group.append(vectorio.Rectangle(
        pixel_shader=palettes['inverted'],
        width=2,
        height=20,
        x=-1,
        y=0
    ))

    max_adjustment = 45

    for i in range(1, max_adjustment//5 + 1):
        height = 10 if i % 2 == 0 else 5

        scale_group.append(vectorio.Rectangle(
            pixel_shader=palettes['inverted'],
            width=1,
            height=height,
            x=i*20,
            y=0
        ))

        scale_group.append(vectorio.Rectangle(
            pixel_shader=palettes['inverted'],
            width=1,
            height=height,
            x=-i*20,
            y=0
        ))

    group.append(scale_group)

    macropad.display.show(group)
    macropad.display.refresh()

    label = display_text.label.Label(
        text='',
        font=terminalio.FONT,
        anchor_point=(0.5, 0),
        anchored_position=(0, -25),
    )
    group.append(label)

    display_minutes = 0
    minutes = 0
    while True:
        key_event = get_key_event()

        if key_event and key_event.pressed:
            if key_state[9]:
                if key_event.key_number == 5:
                    minutes += 5

                if key_event.key_number == 8:
                    minutes -= 5
            else:
                if key_event.key_number == 5:
                    minutes += 1

                if key_event.key_number == 8:
                    minutes -= 1

        minutes = min(max_adjustment, max(-max_adjustment, minutes))

        if key_event and key_event.pressed:
            if key_event.key_number == 2:
                state['name'] = 'toggl_send_adjust_time'
                state['toggl_adjust_minutes'] = minutes
                return

            if key_event.key_number == 1:
                state['name'] = 'toggl'
                return

        display_minutes += (minutes - display_minutes) * 0.3

        label.text = str(minutes)
        scale_group.x = -round(display_minutes * 4)

        macropad.display.refresh()

def toggl_send_adjust_time():
    global state

    send_message(dict(
        kind='adjustTime',
        minutes=state['toggl_adjust_minutes'],
    ))

    message = wait_for_reply_animated(3, gradients_50['light_blue'])
    check_response(message, 3, colors_50['light_blue'])

    reset_activity_timer()
    state['name'] = 'toggl'

def unicorn():
    global state

    clear_pixels()
    macropad.pixels[0] = colors_50['light_blue'].pack()
    macropad.pixels[1] = colors_50['red'].pack()
    macropad.pixels[2] = colors_50['white'].pack()
    macropad.pixels[3] = colors_50['yellow'].pack()

    set_toolbar_pixels()
    macropad.pixels.show()

    group = displayio.Group()
    macropad.display.show(group)
    macropad.display.refresh()

    while True:
        if check_activity_timeout('unicorn'):
            break

        get_message()
        key_event = get_key_event()

        if key_event and key_event.pressed:
            if key_state[11]:
                state['name'] = 'apps'
                state['apps_source_name'] = 'unicorn'
                break

            if key_event.key_number == 0:
                state['name'] = 'unicorn_send_read_inbox'
                break

            if key_event.key_number == 1:
                state['name'] = 'unicorn_send_clear_inbox'
                break

            if key_event.key_number == 2:
                state['name'] = 'unicorn_send_start_clock'
                break

            if key_event.key_number == 3:
                state['name'] = 'unicorn_countdown'
                break

def unicorn_send_read_inbox():
    global state

    send_message(dict(kind='readInbox'))

    message = wait_for_reply_animated(0, gradients_50['light_blue'])
    check_response(message, 0, colors_50['light_blue'])

    reset_activity_timer()
    state['name'] = 'unicorn'

def unicorn_send_clear_inbox():
    global state

    send_message(dict(kind='clearInbox'))

    message = wait_for_reply_animated(1, gradients_50['red'])
    check_response(message, 1, colors_50['red'])

    reset_activity_timer()
    state['name'] = 'unicorn'

def unicorn_send_start_clock():
    global state

    send_message(dict(kind='startClock'))

    message = wait_for_reply_animated(2, gradients_50['white'])
    check_response(message, 2, colors_50['white'])

    reset_activity_timer()
    state['name'] = 'unicorn'

def unicorn_countdown():
    global state

    clear_pixels()
    macropad.pixels[1] = colors_50['red'].pack()
    macropad.pixels[2] = colors_50['green'].pack()
    macropad.pixels[5] = colors_50['white'].pack()
    macropad.pixels[8] = colors_50['white'].pack()
    macropad.pixels[9] = colors_50['white'].pack()
    macropad.pixels.show()

    group = displayio.Group(
        x=macropad.display.width//2,
        y=macropad.display.height//2,
    )

    group.append(vectorio.Polygon(
        pixel_shader=palettes['inverted'],
        points=[
            (0, 0),
            (-4, -4),
            (4, -4),
        ],
        x=0,
        y=-5,
    ))

    scale_group = displayio.Group()

    scale_group.append(vectorio.Rectangle(
        pixel_shader=palettes['inverted'],
        width=2,
        height=20,
        x=-1,
        y=0
    ))

    max_adjustment = 60

    for i in range(1, max_adjustment//5 + 1):
        height = 10 if i % 2 == 0 else 5

        scale_group.append(vectorio.Rectangle(
            pixel_shader=palettes['inverted'],
            width=1,
            height=height,
            x=i*20,
            y=0
        ))

        scale_group.append(vectorio.Rectangle(
            pixel_shader=palettes['inverted'],
            width=1,
            height=height,
            x=-i*20,
            y=0
        ))

    group.append(scale_group)

    macropad.display.show(group)
    macropad.display.refresh()

    label = display_text.label.Label(
        text='',
        font=terminalio.FONT,
        anchor_point=(0.5, 0),
        anchored_position=(0, -25),
    )
    group.append(label)

    display_minutes = 0
    minutes = 0
    while True:
        key_event = get_key_event()

        if key_event and key_event.pressed:
            if key_state[9]:
                if key_event.key_number == 5:
                    minutes += 5

                if key_event.key_number == 8:
                    minutes -= 5
            else:
                if key_event.key_number == 5:
                    minutes += 1

                if key_event.key_number == 8:
                    minutes -= 1

        minutes = min(max_adjustment, max(-max_adjustment, minutes))

        if key_event and key_event.pressed:
            if key_event.key_number == 1:
                state['name'] = 'unicorn'
                return

            if key_event.key_number == 2:
                send_message(dict(
                    kind='startCountdown',
                    minutes=minutes,
                ))

                message = wait_for_reply_animated(3, gradients_50['yellow'])
                check_response(message, 3, colors_50['yellow'])

                reset_activity_timer()
                state['name'] = 'unicorn'
                return

        display_minutes += (minutes - display_minutes) * 0.3

        label.text = str(minutes)
        scale_group.x = -round(display_minutes * 4)

        macropad.display.refresh()

def bluetooth():
    global state

    clear_pixels()
    macropad.pixels[0] = colors_50['white'].pack()
    macropad.pixels[1] = colors_50['gray'].pack()

    set_toolbar_pixels()
    macropad.pixels.show()

    group = displayio.Group()
    macropad.display.show(group)
    macropad.display.refresh()

    while True:
        if check_activity_timeout('bluetooth'):
            break

        get_message()
        key_event = get_key_event()

        if key_event and key_event.pressed:
            if key_state[11]:
                state['name'] = 'apps'
                state['apps_source_name'] = 'bluetooth'
                break

            if key_event.key_number == 0:
                state['name'] = 'bluetooth_send_switch_bose_mac'
                break

            if key_event.key_number == 1:
                state['name'] = 'bluetooth_send_switch_bose_fractal'
                break

def bluetooth_send_switch_bose_mac():
    global state

    send_message(dict(
        kind='switchBoseDevices',
        devices=[
            'A4:83:E7:C9:0F:E3',
            'D4:3A:2C:99:00:E6',
        ],
    ))

    message = wait_for_reply_animated(0, gradients_50['white'], 20000)
    check_response(message, 0, colors_50['white'])

    reset_activity_timer()
    state['name'] = 'bluetooth'

def bluetooth_send_switch_bose_fractal():
    global state

    send_message(dict(
        kind='switchBoseDevices',
        devices=[
            '80:B6:55:F7:40:76',
            'D4:3A:2C:99:00:E6',
        ],
    ))

    message = wait_for_reply_animated(1, gradients_50['gray'], 20000)
    check_response(message, 1, colors_50['gray'])

    reset_activity_timer()
    state['name'] = 'bluetooth'

def servo():
    global state

    clear_pixels()
    set_toolbar_pixels()
    macropad.pixels.show()

    while True:
        get_message()

        if get_x_axis_diff() != 0:
            global last_x_axis

            send_x_axis(last_x_axis)

        if get_y_axis_diff() != 0:
            global last_y_axis

            send_y_axis(last_y_axis)

        key_event = get_key_event()

        if key_event and key_event.pressed:
            if key_state[11]:
                state['name'] = 'apps'
                state['apps_source_name'] = 'servo'
                break


initial_state = dict(
    name='startup',
    options=[''],
    selected_option_index=0,
    options_group=displayio.Group(),

    apps_source_name=None,

    sleep_next_name=None,

    toggl_index=0,
    toggl_options=[''],
    toggl_options_loaded=False,
    toggl_options_group=displayio.Group(),
    toggl_adjust_minutes=0,
)

state_handlers = dict(
    startup=startup,
    apps=apps,
    sleep=sleep,

    toggl=toggl,
    toggl_get_time_entries=toggl_get_time_entries,
    toggl_send_start=toggl_send_start,
    toggl_send_stop=toggl_send_stop,
    toggl_adjust_time=toggl_adjust_time,
    toggl_send_adjust_time=toggl_send_adjust_time,
    toggl_send_continue=toggl_send_continue,

    unicorn=unicorn,
    unicorn_send_read_inbox=unicorn_send_read_inbox,
    unicorn_send_clear_inbox=unicorn_send_clear_inbox,
    unicorn_send_start_clock=unicorn_send_start_clock,
    unicorn_countdown=unicorn_countdown,

    bluetooth=bluetooth,
    bluetooth_send_switch_bose_mac=bluetooth_send_switch_bose_mac,
    bluetooth_send_switch_bose_fractal=bluetooth_send_switch_bose_fractal,

    servo=servo,
)

display_wake()
macropad.display.auto_refresh = False
macropad.pixels.auto_write = False

state = initial_state

while True:
    print('#', state['name'])
    state_handlers[str(state['name'])]()
