from flask import Flask, render_template, request, redirect
from PIL import Image, ImageOps
from inky import InkyE673
import io
import queue
import threading
import time
import RPi.GPIO as GPIO

LED_PIN = 13

GPIO.setmode(GPIO.BCM)
GPIO.setup(LED_PIN, GPIO.OUT)

app = Flask(__name__)

inky = InkyE673()

inky_queue = queue.Queue()

@app.route('/', methods=['GET', 'POST'])
def index():
    if request.method == 'GET':
        return render_template('index.html')

    image = ImageOps.fit(
            ImageOps.exif_transpose(
                Image.open(io.BytesIO(request.files['image'].read()))
            )
            .transpose(Image.Transpose.ROTATE_270),
        inky.resolution,
        method=Image.LANCZOS,
    )

    inky_queue.put(image)

    return redirect(request.path, code=302)

def run_inky_queue():
    while True:
        try:
            image = inky_queue.get()

            GPIO.output(LED_PIN, GPIO.HIGH)

            inky.set_image(image, saturation=0.8)
            inky.show()

            GPIO.output(LED_PIN, GPIO.LOW)

            time.sleep(300)
        except:
            app.logger.error('= inky thread error')

threading.Thread(target=run_inky_queue, daemon=True).start()
