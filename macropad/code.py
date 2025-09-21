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

neoslider = Seesaw(i2c, 0x30)
potentiometer = AnalogInput(neoslider, 18)
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


pcf = PCF.PCF8591(i2c)
pcf_in_0 = AnalogIn(pcf, PCF.A0)
pcf_in_3 = AnalogIn(pcf, PCF.A3)

# print('hello')

# while True:
    # raw_value = pcf_in_0.value
    # scaled_value = (raw_value / 65535) * pcf_in_0.reference_voltage
    # raw_value_x = pcf_in_3.value
    # scaled_value_x = (raw_value_x / 65535) * pcf_in_3.reference_voltage

    # # print((scaled_value, scaled_value_x))
    # # print((raw_value, raw_value_x, pcf_in_0.reference_voltage))

    # time.sleep(0.06)

macropad = MacroPad()

def display_description():
    text_lines[0].text = description
    text_lines.show()

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

def send_heartbeat():
    print('->', 'heartbeat')
    usb_cdc.data.write(bytes('h\n', 'utf-8'))
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

    return message

last_activity = time.time()

def reset_activity_timer():
    global last_activity
    last_activity = time.time()

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

black = fancy.CHSV(0.0, 0.0, 0.0)
white = fancy.CHSV(0.0, 0.0, 1.0)
green = fancy.CRGB(0.0, 1.0, 0.0)
red = fancy.CRGB(1.0, 0.0, 0.0)
yellow = fancy.CRGB(1.0, 1.0, 0.0)
magenta = fancy.CRGB(1.0, 0.0, 1.0)
light_blue = fancy.unpack(0x89CFF0)

def create_palette(foreground, background):
    palette = displayio.Palette(2)
    palette[0] = background.pack()
    palette[1] = foreground.pack()
    return palette;

palettes = dict(
    normal=create_palette(foreground=white, background=black),
    selected=create_palette(foreground=black, background=white),
)

def gamma_adjust(value):
    return fancy.gamma_adjust(value, gamma_value=2.7, brightness=0.3)

colors = dict(
    start=green,
    stop=red,
    load=yellow,
    error=magenta,
    adjust_time=light_blue,
    switch_bose=white,
    shift=white,
    read_inbox=white,
    clear_inbox=white,
    start_clock=white,
    up=white,
    down=white,
)

colors_50 = { k: fancy.mix(v, black, 0.5) for k, v in colors.items() }

command_colors = { k: gamma_adjust(v) for k, v in colors_50.items() }

error_color = gamma_adjust(colors['error'])

start_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['start'])], 25),
)

stop_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['stop'])], 25),
)

load_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['load'])], 25),
)

adjust_time_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['adjust_time'])], 25),
)

switch_bose_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['switch_bose'])], 25),
)

read_inbox_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['read_inbox'])], 25),
)

clear_inbox_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['clear_inbox'])], 25),
)

start_clock_gradient = gamma_adjust(
    fancy.expand_gradient([(0.0, black), (1.0, colors_50['start_clock'])], 25),
)

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

def show_error():
    macropad.pixels.fill(error_color.pack())
    macropad.pixels.show()
    time.sleep(0.3)
    clear_pixels()

def sleep(state):
    while macropad.pixels.brightness > 0:
        macropad.pixels.brightness = max(0, macropad.pixels.brightness - 0.05)
        macropad.pixels.show()
        macropad.display.brightness = max(0, macropad.display.brightness - 0.05)
        time.sleep(0.02)

    display_sleep();

    key_event = None
    encoder_diff = 0
    potentiometer_diff = 0
    while True:
        send_heartbeat()

        key_event = get_key_event(peek=True)
        distance = get_distance()
        encoder_diff = get_encoder_diff(peek=True)
        potentiometer_diff = get_potentiometer_diff(peek=True)
        x_axis_diff = get_x_axis_diff(peek=True)
        y_axis_diff = get_y_axis_diff(peek=True)

        if key_event:
            break

        if distance < 255:
            break

        if encoder_diff != 0:
            break

        if abs(potentiometer_diff) > 5:
            break

        if x_axis_diff != 0 or y_axis_diff != 0:
            break

        last_activity_diff = time.time() - last_activity

        if last_activity_diff > 60:
            light_diff = get_light_diff()
        else:
            light_diff = 0

        #if abs(light_diff) > 0.5 and distance_sensor.range < 255:
        #    reset_activity_timer()
        #    break

        if last_activity_diff > 240:
            set_distance_delay(None)
        elif last_activity_diff > 120:
            set_distance_delay(5000)
        elif last_activity_diff > 60:
            set_distance_delay(2000)
        elif last_activity_diff > 30:
            set_distance_delay(1000)

    reset_activity_timer()
    set_distance_delay(250)
    display_wake()

    while macropad.pixels.brightness < 1:
        macropad.pixels.brightness = min(1, macropad.pixels.brightness + 0.05)
        macropad.pixels.show()
        macropad.display.brightness = min(1, macropad.display.brightness + 0.5)
        time.sleep(0.01)

    macropad.pixels.brightness = 1
    macropad.pixels.show()
    macropad.display.brightness = 1

    return dict(name='command')

def draw_options(state):
    options = state['options']
    selected_option_index = state['selected_option_index']

    options_group = displayio.Group()

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

        options_group.append(label_tile_grid)

        selected = i == selected_option_index
        set_option_tile_grid_selected(label_tile_grid, selected)

    macropad.display.show(options_group)
    macropad.display.refresh()

    return dict(name='command', options_group=options_group)

def command(state):
    options = state['options']
    options_group = state['options_group']
    selected_option_index = state['selected_option_index']

    clear_pixels()
    macropad.pixels[0] = command_colors['load'].pack()
    macropad.pixels[1] = command_colors['stop'].pack()
    macropad.pixels[2] = command_colors['start'].pack()
    macropad.pixels[3] = command_colors['adjust_time'].pack()
    macropad.pixels[4] = command_colors['switch_bose'].pack()
    macropad.pixels[5] = command_colors['switch_bose'].pack()
    macropad.pixels[6] = command_colors['up'].pack()
    macropad.pixels[7] = command_colors['up'].pack()
    macropad.pixels[8] = command_colors['up'].pack()
    macropad.pixels[9] = command_colors['shift'].pack()
    macropad.pixels[10] = command_colors['up'].pack()
    macropad.pixels[11] = command_colors['down'].pack()
    macropad.pixels.show()

    macropad.display.show(options_group)
    macropad.display.refresh()

    while True:
        if time.time() - last_activity > 3:
            return dict(
                name='sleep',
                selected_option_index=selected_option_index
            )

        get_message()
        get_distance()
        key_event = get_key_event()
        selected_diff = get_encoder_diff()

        potentiometer_diff = get_potentiometer_diff()
        x_axis_diff = get_x_axis_diff()
        y_axis_diff = get_y_axis_diff()

        if potentiometer_diff != 0:
            send_potentiometer(last_potentiometer)

        if x_axis_diff != 0:
            send_x_axis(last_x_axis)

        if y_axis_diff != 0:
            send_y_axis(last_y_axis)

        if key_event and key_event.key_number == 8 and key_event.pressed:
            selected_diff -= 1

        if key_event and key_event.key_number == 11 and key_event.pressed:
            selected_diff += 1

        next_selected_option_index = (selected_option_index + selected_diff) % len(options) # TODO crash division 0

        if key_event and key_event.key_number == 8 and key_event.pressed and key_state[9]:
            next_selected_option_index = 0

        if key_event and key_event.key_number == 11 and key_event.pressed and key_state[9]:
            next_selected_option_index = len(options) - 1

        if next_selected_option_index != selected_option_index:
            set_option_tile_grid_selected(
                options_group[selected_option_index],
                False
            )

            set_option_tile_grid_selected(
                options_group[next_selected_option_index],
                True
            )

            selected_option_index = next_selected_option_index

            page = int(selected_option_index / 4)
            target_options_group_y = -page*64
            display_options_group_y = options_group.y

            macropad.display.refresh()

            while options_group.y != target_options_group_y:
                display_options_group_y += (
                    (target_options_group_y - display_options_group_y) * 0.5
                )
                options_group.y = round(display_options_group_y)
                macropad.display.refresh()

        if key_event and key_event.key_number == 2 and key_event.pressed:
            if key_state[9]:
                return dict(
                    name='send_continue',
                    selected_option_index=selected_option_index
                )
            else:
                return dict(
                    name='send_start',
                    selected_option_index=selected_option_index
                )

        if key_event and key_event.key_number == 1 and key_event.pressed:
            return dict(
                name='send_stop',
                selected_option_index=selected_option_index
            )

        if key_event and key_event.key_number == 0 and key_event.pressed:
            return dict(
                name='get_time_entries',
                selected_option_index=selected_option_index
            )

        if key_event and key_event.key_number == 3 and key_event.pressed:
            return dict(
                name='adjust_time',
                selected_option_index=selected_option_index
            )

        if key_event and key_event.key_number == 4 and key_event.pressed:
            return dict(
                name='send_switch_bose_mac',
                selected_option_index=selected_option_index
            )

        if key_event and key_event.key_number == 5 and key_event.pressed:
            return dict(
                name='send_switch_bose_fractal',
                selected_option_index=selected_option_index
            )

        if key_event and key_event.key_number == 6 and key_event.pressed:
            return dict(
                name='send_read_inbox',
                selected_option_index=selected_option_index
            )

        if key_event and key_event.key_number == 7 and key_event.pressed:
            return dict(
                name='send_clear_inbox',
                selected_option_index=selected_option_index
            )

        if key_event and key_event.key_number == 10 and key_event.pressed:
            return dict(
                name='send_start_clock',
                selected_option_index=selected_option_index
            )

def adjust_time(state):
    clear_pixels()
    macropad.pixels[1] = command_colors['stop'].pack()
    macropad.pixels[2] = command_colors['start'].pack()
    macropad.pixels[8] = command_colors['up'].pack()
    macropad.pixels[9] = command_colors['shift'].pack()
    macropad.pixels[11] = command_colors['down'].pack()
    macropad.pixels.show()

    group = displayio.Group(
        x=macropad.display.width//2,
        y=macropad.display.height//2,
    )

    group.append(vectorio.Polygon(
        pixel_shader=palettes['selected'],
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
        pixel_shader=palettes['selected'],
        width=2,
        height=20,
        x=-1,
        y=0
    ))

    max_adjustment = 45

    for i in range(1, max_adjustment//5 + 1):
        height = 10 if i % 2 == 0 else 5

        scale_group.append(vectorio.Rectangle(
            pixel_shader=palettes['selected'],
            width=1,
            height=height,
            x=i*20,
            y=0
        ))

        scale_group.append(vectorio.Rectangle(
            pixel_shader=palettes['selected'],
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
        encoder_diff = get_encoder_diff()

        if key_event and key_event.key_number == 8 and key_event.pressed and not key_state[9]:
            minutes += 1

        if key_event and key_event.key_number == 11 and key_event.pressed and not key_state[9]:
            minutes -= 1

        if key_event and key_event.key_number == 8 and key_event.pressed and key_state[9]:
            minutes += 5

        if key_event and key_event.key_number == 11 and key_event.pressed and key_state[9]:
            minutes -= 5

        minutes = min(max_adjustment, max(-max_adjustment,
            minutes + encoder_diff
        ))

        display_minutes += (minutes - display_minutes) * 0.3

        label.text = str(minutes)
        scale_group.x = -round(display_minutes * 4)

        macropad.display.refresh()

        if key_event and key_event.key_number == 2 and key_event.pressed:
            return dict(name='send_adjust_time', adjust_minutes=minutes)

        if key_event and key_event.key_number == 1 and key_event.pressed:
            return dict(name='command')

def send_continue(state):
    send_message(dict(
        kind='continueTimeEntry',
    ))

    message = wait_for_reply_animated(2, start_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[2] = command_colors['start'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_start(state):
    options = state['options']
    selected_option_index = state['selected_option_index']

    send_message(dict(
        kind='startTimeEntry',
        timeEntry=dict(
            description=options[selected_option_index],
        ),
    ))

    message = wait_for_reply_animated(2, start_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[2] = command_colors['start'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_switch_bose_mac(state):
    send_message(dict(
        kind='switchBoseDevices',
        devices=[
            'A4:83:E7:C9:0F:E3',
            'D4:3A:2C:99:00:E6',
        ],
    ))

    message = wait_for_reply_animated(4, switch_bose_gradient, 20000)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[4] = command_colors['switch_bose'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_switch_bose_fractal(state):
    send_message(dict(
        kind='switchBoseDevices',
        devices=[
            '80:B6:55:F7:40:76',
            'D4:3A:2C:99:00:E6',
        ],
    ))

    message = wait_for_reply_animated(5, switch_bose_gradient, 20000)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[5] = command_colors['switch_bose'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_read_inbox(state):
    send_message(dict(kind='readInbox'))

    message = wait_for_reply_animated(6, read_inbox_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[6] = command_colors['read_inbox'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_clear_inbox(state):
    send_message(dict(kind='clearInbox'))

    message = wait_for_reply_animated(7, clear_inbox_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[7] = command_colors['clear_inbox'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_start_clock(state):
    send_message(dict(kind='startClock'))

    message = wait_for_reply_animated(10, start_clock_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[10] = command_colors['start_clock'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_stop(state):
    send_message(dict(kind='stopTimeEntry'))

    message = wait_for_reply_animated(1, stop_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[1] = command_colors['stop'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def send_adjust_time(state):
    adjust_minutes = state['adjust_minutes']
    send_message(dict(kind='adjustTime', minutes=adjust_minutes))

    message = wait_for_reply_animated(3, adjust_time_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        macropad.pixels[3] = command_colors['adjust_time'].pack()
        macropad.pixels.show()
        time.sleep(0.1)
        clear_pixels()
        time.sleep(0.1)

    reset_activity_timer()
    return dict(name='command')

def get_time_entries(state):
    send_message(dict(kind='getTimeEntries'))

    entries = []

    clear_pixels()
    start = supervisor.ticks_ms()
    while True:
        position = supervisor.ticks_ms() - start

        color = fancy.palette_lookup(load_gradient, position / 1000)
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
        return dict(name='command')

    options = list(set(entries))
    options.sort()

    if not len(options):
        options.append('')

    selected_option_index = 0
    clear_pixels()
    reset_activity_timer()
    return dict(
        name='draw_options',
        options=options,
        selected_option_index=selected_option_index
    )

def set_option_tile_grid_selected(tile_grid, selected):
    palette = palettes['selected' if selected else 'normal']
    tile_grid.pixel_shader = palette

send_heartbeat()
display_wake()

macropad.display.auto_refresh = False
macropad.pixels.auto_write = False

state_handlers = dict(
    sleep=sleep,
    get_time_entries=get_time_entries,
    draw_options=draw_options,
    command=command,
    adjust_time=adjust_time,
    send_continue=send_continue,
    send_start=send_start,
    send_stop=send_stop,
    send_adjust_time=send_adjust_time,
    send_switch_bose_mac=send_switch_bose_mac,
    send_switch_bose_fractal=send_switch_bose_fractal,
    send_read_inbox=send_read_inbox,
    send_clear_inbox=send_clear_inbox,
    send_start_clock=send_start_clock,
)

state = dict(
    name='get_time_entries',
    options=[''],
    selected_option_index=0,
    options_group=displayio.Group(),
)

while True:
    print('#', state['name'])
    result = state_handlers[state['name']](state)
    state = dict(state, **result)
