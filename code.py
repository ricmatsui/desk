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

def get_key_event(peek=False):
    global key_event_buffer

    if len(key_event_buffer):
        key_event = key_event_buffer.pop(0)
    else:
        key_event = macropad.keys.events.get()

    if key_event is not None:
        reset_activity_timer()

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

def wait_for_reply_animated(gradient):
    start = supervisor.ticks_ms()
    while True:
        position = supervisor.ticks_ms() - start

        color = fancy.palette_lookup(gradient, position / 1000)
        macropad.pixels.fill(color.pack())
        macropad.pixels.show()

        message = get_message()

        if message is not None:
            return message

        if position > 5000:
            return None

def show_error():
    macropad.pixels.fill(error_color.pack())
    macropad.pixels.show()
    time.sleep(1)
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
    while not key_event and encoder_diff == 0:
        key_event = get_key_event(peek=True)
        encoder_diff = get_encoder_diff(peek=True)

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
            y=i*15,
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
        key_event = get_key_event()
        encoder_diff = get_encoder_diff()

        next_selected_option_index = (selected_option_index + encoder_diff) % len(options)

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
            options_group.y = -page*60
            macropad.display.refresh()

        if key_event and key_event.key_number == 2 and key_event.pressed:
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

def adjust_time(state):
    clear_pixels()
    macropad.pixels[1] = command_colors['stop'].pack()
    macropad.pixels[2] = command_colors['start'].pack()
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

    minutes = 0
    while True:
        key_event = get_key_event()
        encoder_diff = get_encoder_diff()

        minutes = min(max_adjustment, max(-max_adjustment,
            minutes + encoder_diff
        ))

        label.text = str(minutes)
        scale_group.x = -minutes * 4

        macropad.display.refresh()

        if key_event and key_event.key_number == 2 and key_event.pressed:
            return dict(name='send_adjust_time', adjust_minutes=minutes)

        if key_event and key_event.key_number == 1 and key_event.pressed:
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

    message = wait_for_reply_animated(start_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        for i in range(2):
            macropad.pixels[2] = command_colors['start'].pack()
            macropad.pixels.show()
            time.sleep(0.2)
            clear_pixels()
            time.sleep(0.2)

    reset_activity_timer()
    return dict(name='command')

def send_stop(state):
    send_message(dict(kind='stopTimeEntry'))

    message = wait_for_reply_animated(stop_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        for i in range(2):
            macropad.pixels[1] = command_colors['stop'].pack()
            macropad.pixels.show()
            time.sleep(0.2)
            clear_pixels()
            time.sleep(0.2)

    reset_activity_timer()
    return dict(name='command')

def send_adjust_time(state):
    adjust_minutes = state['adjust_minutes']
    send_message(dict(kind='adjustTime', minutes=adjust_minutes))

    message = wait_for_reply_animated(adjust_time_gradient)

    if not message or message['kind'] != 'success':
        show_error()
    else:
        for i in range(2):
            macropad.pixels[3] = command_colors['adjust_time'].pack()
            macropad.pixels.show()
            time.sleep(0.2)
            clear_pixels()
            time.sleep(0.2)

    reset_activity_timer()
    return dict(name='command')

def get_time_entries(state):
    send_message(dict(kind='getTimeEntries'))

    entries = []

    start = supervisor.ticks_ms()
    while True:
        position = supervisor.ticks_ms() - start

        color = fancy.palette_lookup(load_gradient, position / 1000)
        macropad.pixels.fill(color.pack())
        macropad.pixels.show()

        message = get_message()

        if message:
            if message['kind'] == 'timeEntry':
                entries.append(message['timeEntry']['description'])
            else:
                break

        if position > 10000:
            break

    if not message or message['kind'] != 'success':
        show_error()
        reset_activity_timer()
        return dict(name='command')

    options = list(set(entries))
    options.sort()
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

display_wake()

macropad.display.auto_refresh = False
macropad.pixels.auto_write = False

state_handlers = dict(
    sleep=sleep,
    get_time_entries=get_time_entries,
    draw_options=draw_options,
    command=command,
    adjust_time=adjust_time,
    send_start=send_start,
    send_stop=send_stop,
    send_adjust_time=send_adjust_time,
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
