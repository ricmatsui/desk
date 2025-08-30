import time
import board
import math
import touchio
import usb_cdc
import digitalio
import neopixel
from rainbowio import colorwheel
import adafruit_fancyled.adafruit_fancyled as fancy

touch_a1 = touchio.TouchIn(board.A1)
button = digitalio.DigitalInOut(board.BUTTON_A)
button.switch_to_input(pull=digitalio.Pull.DOWN)
pixels = neopixel.NeoPixel(board.NEOPIXEL, 10, brightness=1.0, auto_write=False)

def send_heartbeat():
    print('->', 'heartbeat')
    usb_cdc.data.write(bytes('c\n', 'utf-8'))
    usb_cdc.data.flush()

def send_readings():
    print('->', 'readings')
    usb_cdc.data.write(bytes('ra1' + str(touch_a1.raw_value) + '\n', 'utf-8'))
    usb_cdc.data.flush()

show_readings = False
time_since_update = 0
previous_button_value = False
current_value = 0.0

OFF = (0, 0, 0)
CYAN = (0, 255, 255)

while True:
    if button.value != previous_button_value and button.value:
        show_readings = not show_readings
    previous_button_value = button.value

    if show_readings:
        value = touch_a1.raw_value / 4096 * 10
        current_value += (value - current_value)/4
        for i in range(10):
            color = fancy.gamma_adjust(
                fancy.CHSV(0.5, 1.0, max(0.0, min(1.0, current_value - i))),
                gamma_value=2.7,
                brightness=0.03
            )
            pixels[i] = color.pack()
        pixels.show()
    else:
        pixels.fill(OFF)
        pixels.show()

    time.sleep(0.01)
    time_since_update += 0.01

    if time_since_update >= 1:
        time_since_update = 0

        send_heartbeat()
        send_readings()


