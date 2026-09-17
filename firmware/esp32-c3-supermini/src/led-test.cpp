// Standalone wiring check: one full-brightness color per second, no Bluetooth.
#include <Arduino.h>

constexpr uint8_t kPins[] = {LED_RED_PIN, LED_YELLOW_PIN, LED_GREEN_PIN};
constexpr const char* kNames[] = {"red", "yellow", "green"};

void setup() {
  Serial.begin(115200);
  for (uint8_t pin : kPins) {
    pinMode(pin, OUTPUT);
    digitalWrite(pin, LOW);
  }
  if (BUILTIN_LED_PIN >= 0) {
    pinMode(BUILTIN_LED_PIN, OUTPUT);
    digitalWrite(BUILTIN_LED_PIN, BUILTIN_LED_ACTIVE_LOW ? HIGH : LOW);
  }
}

void loop() {
  static unsigned color = 0;
  for (unsigned i = 0; i < 3; ++i) {
    digitalWrite(kPins[i], i == color ? HIGH : LOW);
  }
  Serial.printf("led-test color=%s gpio=%u duration_ms=1000\n", kNames[color],
                kPins[color]);
  if (BUILTIN_LED_PIN >= 0)
    digitalWrite(BUILTIN_LED_PIN, BUILTIN_LED_ACTIVE_LOW ? LOW : HIGH);
  delay(100);
  Serial.printf("pin_levels red=%d yellow=%d green=%d\n", digitalRead(kPins[0]),
                digitalRead(kPins[1]), digitalRead(kPins[2]));
  if (BUILTIN_LED_PIN >= 0)
    digitalWrite(BUILTIN_LED_PIN, BUILTIN_LED_ACTIVE_LOW ? HIGH : LOW);
  delay(900);
  color = (color + 1) % 3;
}
