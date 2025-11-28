#include <Adafruit_ThinkInk.h>
#include <Adafruit_PWMServoDriver.h>
#include <Adafruit_IS31FL3741.h>
#include <ArduinoJson.h>
#include <base64.hpp>

// Feather RP2040 ThinkInk
#define EPD_DC 10 // ThinkInk 24-pin connector DC
#define EPD_CS 9 // ThinkInk 24-pin connector CS
#define EPD_BUSY -1 // ThinkInk 24-pin connector Busy
#define SRAM_CS -1 // use onboard RAM
#define EPD_RESET -1 // ThinkInk 24-pin connector Reset
#define EPD_SPI &SPI

#define COLOR1 EPD_BLACK
#define COLOR2 EPD_LIGHT
#define COLOR3 EPD_DARK

ThinkInk_290_Grayscale4_T5 display(
  EPD_DC, EPD_RESET, EPD_CS, SRAM_CS, EPD_BUSY, EPD_SPI
);

const uint8_t PROGMEM EPD_COLORS[] = {
    EPD_BLACK,
    EPD_DARK,
    EPD_LIGHT,
    EPD_WHITE
};

Adafruit_IS31FL3741_QT_buffered matrix;
TwoWire *i2c = &Wire;

#define SERVO_HOLD false

#define SERVO_X_PIN 4
#define SERVO_Y_PIN 5
#define LED_RED_PIN 6
#define LED_BLUE_PIN 7
#define LIGHT_MAX_VALUE 8190

const uint8_t PROGMEM GAMMA_8[] = {
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  1,  1,  1,  1,
    1,  1,  1,  1,  1,  1,  1,  1,  1,  2,  2,  2,  2,  2,  2,  2,
    2,  3,  3,  3,  3,  3,  3,  3,  4,  4,  4,  4,  4,  5,  5,  5,
    5,  6,  6,  6,  6,  7,  7,  7,  7,  8,  8,  8,  9,  9,  9, 10,
   10, 10, 11, 11, 11, 12, 12, 13, 13, 13, 14, 14, 15, 15, 16, 16,
   17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 24, 24, 25,
   25, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36,
   37, 38, 39, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 50,
   51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 66, 67, 68,
   69, 70, 72, 73, 74, 75, 77, 78, 79, 81, 82, 83, 85, 86, 87, 89,
   90, 92, 93, 95, 96, 98, 99,101,102,104,105,107,109,110,112,114,
  115,117,119,120,122,124,126,127,129,131,133,135,137,138,140,142,
  144,146,148,150,152,154,156,158,160,162,164,167,169,171,173,175,
  177,180,182,184,186,189,191,193,196,198,200,203,205,208,210,213,
  215,218,220,223,225,228,231,233,236,239,241,244,247,249,252,255 };

Adafruit_PWMServoDriver pwm = Adafruit_PWMServoDriver();

int currentValue = 0;
int targetValue = 0;
int speed = 0;

// Center: 300
int servoXCurrentValue = 300;
int servoXTargetValue = 300;
int servoXSpeed = 1;

// Center: 300
// 45deg up: 200
// 45deg down: 400
int servoYCurrentValue = 300;
int servoYTargetValue = 300;
int servoYSpeed = 1;
//int servoYSpeed = 0;

unsigned long startTime = 0;

// Using color format 565
#define GREEN    0x07E0
#define BLUE     0x001F
#define WHITE    0xFFFF
#define BLACK    0x0000

#define GRID_WIDTH 13
#define GRID_HEIGHT 9
#define GRID_SIZE (GRID_WIDTH * GRID_HEIGHT)

uint16_t sandGrid[GRID_SIZE];
bool updated[GRID_SIZE];
unsigned long lastAdditionTime = 0;
const unsigned long ADDITION_INTERVAL = 60 * 1000;
int slowDown = 0;
bool animating = false;

void setup() {
    display.begin(THINKINK_GRAYSCALE4);

    pwm.begin();
    pwm.setOscillatorFrequency(27000000);
    pwm.setPWMFreq(50);

    setLight(currentValue);

    i2c->setClock(800000);
    matrix.begin(IS3741_ADDR_DEFAULT, i2c);
    matrix.setLEDscaling(0x0F);
    matrix.setGlobalCurrent(0x01);
    matrix.fill(0);
    matrix.enable(false);
    matrix.setRotation(2);
    matrix.setTextWrap(false);

    startTime = micros();

    resetSand();

    Serial.begin(115200);
}

uint16_t gammaCorrected(uint16_t input) {
    if (input == 4095) {
        return input;
    }

    float scaledInput = (float)input / 4095.0 * 255.0;

    uint8_t lowerIndex = (uint8_t)scaledInput;
    uint8_t upperIndex = lowerIndex + 1;

    float lowerValue = GAMMA_8[lowerIndex];
    float upperValue = GAMMA_8[upperIndex];

    float fraction = scaledInput - lowerIndex;
    float interpolatedValue = lowerValue + fraction * (upperValue - lowerValue);

    return (uint16_t)(interpolatedValue / 255.0 * 4095.0);
}

void setLight(int value) {
    //servoYTargetValue = 400 - (value / 8190.0 * 200.0);

    if (value > 4095) {
        pwm.setPin(LED_RED_PIN, 4095);
        pwm.setPin(LED_BLUE_PIN, gammaCorrected(min(4095, value - 4095)));
    } else {
        pwm.setPin(LED_RED_PIN, gammaCorrected(max(0, value)));
        pwm.setPin(LED_BLUE_PIN, 0);
    }
}

char messageBuffer[1024];
bool messageReady = false;
int messageBufferIndex = 0;

void resetSand() {
    for (int i = 0; i < GRID_SIZE; i++) {
        sandGrid[i] = BLACK;
    }
}

void loop() {
    int elapsedTime = micros() - startTime;
    if (elapsedTime < 16666) {
        delayMicroseconds(max(0, 16666 - elapsedTime));
    }
    startTime = micros();

    slowDown++;

    if (slowDown > 10) {
        slowDown = 0;

        int emptyCount = 0;
        for (int i = 0; i < GRID_SIZE; i++) {
            if (sandGrid[i] == BLACK) {
                emptyCount++;
            }
        }
        
        if (emptyCount < 13) {
            for (int i = 0; i < GRID_SIZE; i++) {
                sandGrid[i] = BLACK;
            }
        }

        updateSand(true);
        
        unsigned long currentTime = millis();

        if (lastAdditionTime == 0 || currentTime - lastAdditionTime >= ADDITION_INTERVAL) {
            int y = 0;
            int x = random(0, GRID_WIDTH);
            
            for (int attempts = 0; attempts < GRID_WIDTH; attempts++) {
                int index = y * GRID_WIDTH + (x + attempts) % GRID_WIDTH;
                if (sandGrid[index] == BLACK) {
                    sandGrid[index] = GREEN;
                    break;
                }
            }
            
            lastAdditionTime = currentTime;
        }
        
        matrix.fill(0);

        for (int i = 0; i < GRID_SIZE; i++) {
            int x = i % GRID_WIDTH;
            int y = i / GRID_WIDTH;
            if (sandGrid[i] == GREEN) {
                matrix.drawPixel(x, y, GREEN);
            }
        }
        
        matrix.show();

        if (currentValue < targetValue) {
            currentValue = min(targetValue, currentValue + speed);
            setLight(currentValue);
        } else if (currentValue > targetValue) {
            currentValue = max(targetValue, currentValue - speed);
            setLight(currentValue);
        }
    }

    if (Serial.availableForWrite() >= 2) {
        Serial.write("t\n");
    }

    while (Serial.available() > 0) {
        if (messageBufferIndex < sizeof(messageBuffer) - 1) {
            int value = Serial.read();

            messageBuffer[messageBufferIndex] = value;
            messageBufferIndex++;

            if (value == '\n') {
                messageBuffer[messageBufferIndex] = '\0';
                messageReady = true;
            }
        } else {
            messageBufferIndex = 0;
            messageReady = false;
        }

        if (messageReady) {
            JsonDocument message;

            DeserializationError error = deserializeJson(message, messageBuffer);

            messageBufferIndex = 0;
            messageReady = false;

            if (error == DeserializationError::Ok) {
                if (message["kind"] == "light") {
                    targetValue = message["targetValue"];
                    speed = message["speed"];
                }

                if (message["kind"] == "servoY") {
                    servoYTargetValue = message["targetValue"];
                    servoYSpeed = message["speed"];
                }

                if (message["kind"] == "servoX") {
                    servoXTargetValue = message["targetValue"];
                    servoXSpeed = message["speed"];
                }

                if (message["kind"] == "startAnimation") {
                    resetSand();
                    lastAdditionTime = 0;
                    matrix.fill(0);
                    matrix.show();
                    matrix.enable(true);
                    animating = true;
                }

                if (message["kind"] == "stopAnimation") {
                    while (true) {
                        int elapsedTime = micros() - startTime;
                        if (elapsedTime < 16666) {
                            delayMicroseconds(max(0, 16666 - elapsedTime));
                        }
                        startTime = micros();

                        slowDown++;

                        if (slowDown > 10) {
                            slowDown = 0;

                            int filledCount = 0;
                            for (int i = 0; i < GRID_SIZE; i++) {
                                if (sandGrid[i] == GREEN) {
                                    filledCount++;
                                }
                            }
                            
                            if (filledCount == 0) {
                                break;
                            }

                            updateSand(false);

                            matrix.fill(0);

                            for (int i = 0; i < GRID_SIZE; i++) {
                                int x = i % GRID_WIDTH;
                                int y = i / GRID_WIDTH;
                                if (sandGrid[i] == GREEN) {
                                    matrix.drawPixel(x, y, GREEN);
                                }
                            }
                            
                            matrix.show();
                        }
                    }

                    matrix.fill(0);
                    matrix.show();
                    matrix.enable(false);
                    animating = false;
                }

                if (message["kind"] == "adjustAnimationTime") {
                    int minutes = message["minutes"];

                    bool remove = minutes < 0;
                    int count = abs(minutes);

                    if (count > 0) {
                        for (int y = 0; y < GRID_HEIGHT; y++) {
                            int indexes[GRID_WIDTH];
                            initArray(indexes, GRID_WIDTH);
                            shuffle(indexes, GRID_WIDTH);
                            
                            for (int i = 0; i < GRID_WIDTH; i++) {
                                int index = y * GRID_WIDTH + indexes[i];

                                if (remove) {
                                    if (sandGrid[index] == GREEN) {
                                        sandGrid[index] = BLACK;

                                        count--;
                                    }
                                } else {
                                    if (sandGrid[index] == BLACK) {
                                        sandGrid[index] = GREEN;

                                        count--;
                                    }
                                }

                                if (count == 0) {
                                    break;
                                }
                            }

                            if (count == 0) {
                                break;
                            }
                        }
                    }
                }

                if (message["kind"] == "displayData") {
                    int offset = message["offset"];
                    
                    uint8_t dataBuffer[1024];
                    String data = message["data"];
                    int dataLength = decode_base64((unsigned char*)data.c_str(), dataBuffer);

                    for (int i = 0; i < dataLength; i++) {
                        uint8_t data = dataBuffer[i];
                        for (int j = 0; j < 4; j++) {
                            int pixelIndex = offset * 4 + i * 4 + j;
                            uint8_t color = (data >> ((3 - j) * 2)) & 0x03;

                            int x = pixelIndex % display.width();
                            int y = pixelIndex / display.width();

                            display.drawPixel(x, y, EPD_COLORS[color]);
                        }
                        
                    }
                }

                if (message["kind"] == "refreshDisplay") {
                    display.display(true); // Display and sleep
                }
            }
        }
    }

    if (servoXCurrentValue < servoXTargetValue) {
        servoXCurrentValue = max(150, min(600, min(servoXTargetValue, servoXCurrentValue + servoXSpeed)));

        if (!SERVO_HOLD) {
            pwm.setPWM(SERVO_X_PIN, 0, servoXCurrentValue);
        }
    } else if (servoXCurrentValue > servoXTargetValue) {
        servoXCurrentValue = max(150, min(600, max(servoXTargetValue, servoXCurrentValue - servoXSpeed)));

        if (!SERVO_HOLD) {
            pwm.setPWM(SERVO_X_PIN, 0, servoXCurrentValue);
        }
    }

    if (servoYCurrentValue < servoYTargetValue) {
        servoYCurrentValue = max(150, min(600, min(servoYTargetValue, servoYCurrentValue + servoYSpeed)));

        if (!SERVO_HOLD) {
            pwm.setPWM(SERVO_Y_PIN, 0, servoYCurrentValue);
        }
    } else if (servoYCurrentValue > servoYTargetValue) {
        servoYCurrentValue = max(150, min(600, max(servoYTargetValue, servoYCurrentValue - servoYSpeed)));

        if (!SERVO_HOLD) {
            pwm.setPWM(SERVO_Y_PIN, 0, servoYCurrentValue);
        }
    }
}

void updateSand(bool floorEnabled) {
    for (int i = 0; i < GRID_SIZE; i++) {
        updated[i] = false;
    }
    
    for (int y = 0; y < GRID_HEIGHT; y++) {
        for (int x = 0; x < GRID_WIDTH; x++) {
            int index = y * GRID_WIDTH + x;
            uint16_t color = sandGrid[index];
            
            if (updated[index]) {
                continue;
            }
            
            if (color == GREEN) {
                if (!floorEnabled) {
                    if (y == GRID_HEIGHT - 1) {
                        if (random(0, 10000) <= 9000) {
                            sandGrid[index] = BLACK;
                            continue;
                        }
                    }
                }

                if (y < GRID_HEIGHT - 1) {
                    int belowIndex = (y + 1) * GRID_WIDTH + x;
                    if (sandGrid[belowIndex] == BLACK) {
                        if (random(0, 10000) <= 9000) {
                            sandGrid[index] = BLACK;
                            sandGrid[belowIndex] = GREEN;
                            updated[belowIndex] = true;
                            continue;
                        }
                    }
                }
                
                if (x > 0) {
                    int leftIndex = y * GRID_WIDTH + (x - 1);
                    if (sandGrid[leftIndex] == BLACK) {
                        if (random(0, 10000) <= 1) {
                            sandGrid[index] = BLACK;
                            sandGrid[leftIndex] = GREEN;
                            updated[leftIndex] = true;
                            continue;
                        }
                    }
                }
                
                if (x < GRID_WIDTH - 1) {
                    int rightIndex = y * GRID_WIDTH + (x + 1);
                    if (sandGrid[rightIndex] == BLACK) {
                        if (random(0, 10000) <= 1) {
                            sandGrid[index] = BLACK;
                            sandGrid[rightIndex] = GREEN;
                            updated[rightIndex] = true;
                            continue;
                        }
                    }
                }
            }
        }
    }
}

void initArray(int *array, int size) {
    for (int i = 0; i < size; i++) {
        array[i] = i;
    }
}

void shuffle(int *array, int size) {
    for (int i = size - 1; i > 0; i--) {
        int j = random(0, i + 1);
        int temp = array[i];
        array[i] = array[j];
        array[j] = temp;
    }
}
