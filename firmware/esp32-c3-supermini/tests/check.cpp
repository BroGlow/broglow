// Host-only contract tests; mocks below compile the real firmware, without
// hardware.

#include <algorithm>
#include <array>
#include <cassert>
#include <cmath>
#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <deque>
#include <string>
#include <vector>

uint32_t now = 0;
uint32_t millis() { return now; }
void delay(uint32_t duration) { now += duration; }
void (*serialAvailableHook)() = nullptr;
struct MockSerial {
  std::deque<char> input;
  std::string output;
  unsigned baud = 0;
  bool flood = false;
  void begin(unsigned value) { baud = value; }
  size_t available() {
    if (serialAvailableHook) serialAvailableHook();
    return input.size();
  }
  int read() {
    char value = input.front();
    input.pop_front();
    if (flood) input.push_back('x');
    return value;
  }
  void print(const char* value) { output += value; }
  void printf(const char* format, ...) {
    char buffer[512];
    va_list args;
    va_start(args, format);
    vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    output += buffer;
  }
} Serial;

constexpr int LOW = 0, HIGH = 1, OUTPUT = 1, INPUT_PULLUP = 2;
int buttonLevel = HIGH;
std::array<int, 22> levels{};
void pinMode(size_t, int) {}
void digitalWrite(size_t pin, int value) { levels.at(pin) = value; }
int digitalRead(size_t pin) { return pin == 9 ? buttonLevel : levels.at(pin); }
// Observe physical pins; channel assignments, PWM frequency and resolution may
// vary.
std::array<int, 6> pwmPins{{-1, -1, -1, -1, -1, -1}};
std::array<unsigned, 6> pwmDuty{}, pwmResolution{};
double ledcSetup(size_t channel, double frequency, unsigned resolution) {
  pwmResolution.at(channel) = resolution;
  return frequency;
}
void ledcWrite(size_t channel, unsigned duty) { pwmDuty.at(channel) = duty; }
void ledcAttachPin(int pin, size_t channel) { pwmPins.at(channel) = pin; }
struct MockEsp {
  uint64_t getEfuseMac() { return 0x244592936314ULL; }
} ESP;

constexpr int ESP_OK = 0, ESP_GATT_PERM_WRITE_ENCRYPTED = 0x20;
constexpr int ESP_LE_AUTH_REQ_SC_BOND = 9, ESP_IO_CAP_NONE = 3;
constexpr int ESP_BLE_ENC_KEY_MASK = 1, ESP_BLE_ID_KEY_MASK = 2;
constexpr int ESP_BLE_ONLY_ACCEPT_SPECIFIED_AUTH_ENABLE = 1;
constexpr int ESP_BLE_SM_ONLY_ACCEPT_SPECIFIED_SEC_AUTH = 6;
struct esp_ble_auth_cmpl_t {
  bool success;
  uint8_t fail_reason;
};
struct esp_ble_bond_dev_t {
  uint8_t bd_addr[6];
};
std::vector<esp_ble_bond_dev_t> mockBonds;
int esp_ble_get_bond_device_num() { return static_cast<int>(mockBonds.size()); }
int esp_ble_get_bond_device_list(int* count, esp_ble_bond_dev_t* output) {
  *count = std::min(*count, static_cast<int>(mockBonds.size()));
  std::memcpy(output, mockBonds.data(), *count * sizeof(*output));
  return ESP_OK;
}
int esp_ble_remove_bond_device(uint8_t* address) {
  for (auto it = mockBonds.begin(); it != mockBonds.end(); ++it) {
    if (std::memcmp(it->bd_addr, address, 6) == 0) {
      mockBonds.erase(it);
      return ESP_OK;
    }
  }
  return -1;
}
bool strictSecurity = false;
int esp_ble_gap_set_security_param(int type, uint8_t* value, size_t length) {
  assert(type == ESP_BLE_SM_ONLY_ACCEPT_SPECIFIED_SEC_AUTH && length == 1);
  strictSecurity = *value == ESP_BLE_ONLY_ACCEPT_SPECIFIED_AUTH_ENABLE;
  return ESP_OK;
}
struct BLESecurityCallbacks {
  virtual uint32_t onPassKeyRequest() = 0;
  virtual void onPassKeyNotify(uint32_t) = 0;
  virtual bool onSecurityRequest() = 0;
  virtual void onAuthenticationComplete(esp_ble_auth_cmpl_t) = 0;
  virtual bool onConfirmPIN(uint32_t) = 0;
  virtual ~BLESecurityCallbacks() = default;
};
struct BLESecurity {
  static inline int auth = 0, capability = -1, keySize = 0, initKey = 0,
                    respKey = 0;
  void setAuthenticationMode(int value) { auth = value; }
  void setCapability(int value) { capability = value; }
  void setKeySize(int value) { keySize = value; }
  void setInitEncryptionKey(int value) { initKey = value; }
  void setRespEncryptionKey(int value) { respKey = value; }
};

using esp_gatt_if_t = uint8_t;
enum esp_gatts_cb_event_t {
  ESP_GATTS_CONNECT_EVT,
  ESP_GATTS_DISCONNECT_EVT,
  ESP_GATTS_SEND_SERVICE_CHANGE_EVT
};
struct esp_ble_gatts_cb_param_t {
  struct {
    uint8_t remote_bda[6];
  } connect;
  struct {
    int status;
  } service_change;
};
using gatts_event_handler = void (*)(esp_gatts_cb_event_t, esp_gatt_if_t,
                                     esp_ble_gatts_cb_param_t*);
uint8_t mockChangeMode = 0, mockChangeModeResult = 0;
extern "C" uint8_t BTA_GATTS_SetServiceChangeMode(uint8_t mode) {
  assert(mode == 1);
  mockChangeMode = mode;
  return mockChangeModeResult;
}
unsigned mockChangeRequests = 0;
esp_gatt_if_t mockChangeInterface = 0;
uint8_t mockChangeAddress[6]{};
int mockChangeRequestResult = 0;
int esp_ble_gatts_send_service_change_indication(esp_gatt_if_t interface,
                                                 uint8_t* address) {
  assert(mockChangeMode == 1 && mockChangeModeResult == 0);
  ++mockChangeRequests;
  mockChangeInterface = interface;
  std::memcpy(mockChangeAddress, address, sizeof(mockChangeAddress));
  return mockChangeRequestResult;
}

struct BLECharacteristic;
struct BLEServer;
struct BLECharacteristicCallbacks {
  virtual void onWrite(BLECharacteristic*) {}
  virtual ~BLECharacteristicCallbacks() = default;
};
struct BLEServerCallbacks {
  virtual void onConnect(BLEServer*) {}
  virtual void onDisconnect(BLEServer*) {}
  virtual ~BLEServerCallbacks() = default;
};
struct BLECharacteristic {
  static constexpr int PROPERTY_WRITE = 1;
  BLECharacteristicCallbacks* callbacks = nullptr;
  std::string value, uuid;
  int properties = 0;
  int permissions = 0;
  void setCallbacks(BLECharacteristicCallbacks* value) { callbacks = value; }
  std::string getValue() { return value; }
  void setValue(const std::string& bytes) { value = bytes; }
  void setAccessPermissions(int value) { permissions = value; }
  bool write(const std::string& bytes, bool encrypted = true) {
    if ((permissions & ESP_GATT_PERM_WRITE_ENCRYPTED) && !encrypted)
      return false;
    value = bytes;
    callbacks->onWrite(this);
    return true;
  }
} characteristic, brightnessCharacteristic;
struct BLEService {
  std::string uuid;
  bool started = false;
  BLECharacteristic* createCharacteristic(const char* value, int properties) {
    auto* target = std::string(value) == "7d2ea28c-f7bd-485a-bd9d-92ad6ecfe93e"
                       ? &brightnessCharacteristic
                       : &characteristic;
    target->uuid = value;
    target->properties = properties;
    return target;
  }
  void start() { started = true; }
} service;
struct BLEServer {
  BLEServerCallbacks* callbacks = nullptr;
  void setCallbacks(BLEServerCallbacks* value) { callbacks = value; }
  BLEService* createService(const char* value) {
    service.uuid = value;
    return &service;
  }
  uint16_t getConnId() { return 42; }
  void disconnect(uint16_t id) {
    assert(id == 42);
    callbacks->onDisconnect(this);
  }
} server;
constexpr uint8_t ESP_BLE_ADV_FLAG_GEN_DISC = 2,
                  ESP_BLE_ADV_FLAG_BREDR_NOT_SPT = 4;
struct BLEUUID {
  std::string value;
  explicit BLEUUID(const char* value) : value(value) {}
};
struct BLEAdvertisementData {
  std::string uuid, name, manufacturer, payload;
  uint8_t flags = 0;
  void add(size_t size) {
    if (payload.size() + size <= 31) payload.append(size, '\0');
  }
  void setFlags(uint8_t value) {
    flags = value;
    add(3);
  }
  void setCompleteServices(BLEUUID value) {
    uuid = value.value;
    add(18);
  }
  void setManufacturerData(std::string value) {
    manufacturer = value;
    add(value.size() + 2);
  }
  void setName(std::string value) {
    name = value;
    add(value.size() + 2);
  }
  std::string getPayload() { return payload; }
};
struct BLEAdvertising {
  BLEAdvertisementData advertisement, scanResponse;
  bool active = false;
  void setAdvertisementData(BLEAdvertisementData& value) {
    advertisement = value;
  }
  void setScanResponseData(BLEAdvertisementData& value) {
    scanResponse = value;
  }
  void start() { active = true; }
} advertising;
struct BLEDevice {
  static inline std::string name;
  static inline BLESecurityCallbacks* security = nullptr;
  static inline gatts_event_handler gattsHandler = nullptr;
  static void init(const char* value) { name = value; }
  static void setCustomGattsHandler(gatts_event_handler value) {
    gattsHandler = value;
  }
  static void setSecurityCallbacks(BLESecurityCallbacks* value) {
    security = value;
  }
  static BLEServer* createServer() { return &server; }
  static BLEAdvertising* getAdvertising() { return &advertising; }
  static void startAdvertising() { advertising.start(); }
};

#define configASSERT(value) assert(value)
constexpr int pdTRUE = 1;
struct MockQueue {
  std::vector<char> data;
  bool pending = false;
};
std::deque<MockQueue> queues;
using QueueHandle_t = MockQueue*;
void (*queueReceiveHook)(QueueHandle_t) = nullptr;
QueueHandle_t xQueueCreate(size_t, size_t size) {
  queues.emplace_back();
  auto& queue = queues.back();
  queue.data.resize(size);
  return &queue;
}
int xQueueOverwrite(QueueHandle_t handle, const void* event) {
  std::memcpy(handle->data.data(), event, handle->data.size());
  handle->pending = true;
  return pdTRUE;
}
int xQueueReceive(QueueHandle_t handle, void* event, unsigned) {
  if (queueReceiveHook) queueReceiveHook(handle);
  if (!handle->pending) return 0;
  std::memcpy(event, handle->data.data(), handle->data.size());
  handle->pending = false;
  return pdTRUE;
}

#include "../src/main.cpp"

void feed(const std::string& bytes) {
  Serial.input.insert(Serial.input.end(), bytes.begin(), bytes.end());
  do {
    loop();
  } while (!Serial.input.empty());
}
void at(uint32_t time) {
  now = time;
  loop();
}
void statusAt(uint32_t time, const std::string& bytes) {
  now = time;
  characteristic.write(bytes);
  loop();
}
void brightnessAt(uint32_t time, uint8_t percent) {
  now = time;
  brightnessCharacteristic.write(std::string(1, static_cast<char>(percent)));
  loop();
}
void previewAt(uint32_t time, uint8_t percent, uint8_t mode = 1) {
  now = time;
  brightnessCharacteristic.write(
      std::string{static_cast<char>(percent), static_cast<char>(mode)});
  loop();
}
void connectPeer() {
  advertising.active = false;
  server.callbacks->onConnect(&server);
}
void disconnectPeer() { server.callbacks->onDisconnect(&server); }
double pinDuty(int pin) {
  for (size_t channel = 0; channel < pwmPins.size(); ++channel) {
    if (pwmPins[channel] == pin)
      return 255.0 * pwmDuty[channel] / ((1u << pwmResolution[channel]) - 1);
  }
  return levels.at(pin) == HIGH ? 255 : 0;
}
void expectLamp(int red, int yellow, int green, int builtin = -1) {
  const auto matches = [](double actual, int expected) {
    return std::abs(actual - expected) <=
           (expected == 0 || expected == 255 ? 0 : 1);
  };
  for (const auto& expected :
       {std::pair<int, int>{0, red}, {1, yellow}, {3, green}})
    assert(matches(pinDuty(expected.first), expected.second));
  if (BUILTIN_LED_PIN < 0) {
    assert(std::find(pwmPins.begin(), pwmPins.end(), 8) == pwmPins.end());
  } else if (builtin >= 0) {
    const auto actual = BUILTIN_LED_ACTIVE_LOW ? 255 - pinDuty(BUILTIN_LED_PIN)
                                               : pinDuty(BUILTIN_LED_PIN);
    assert(matches(actual, builtin));
  }
}
void checkAt(uint32_t time, int red, int yellow, int green, int builtin) {
  at(time);
  expectLamp(red, yellow, green, builtin);
}
void bondedBoot() {
  mockBonds = {{{1, 2, 3, 4, 5, 6}}};
  // Existing scenarios use time zero after startup; exercise clock wrap too.
  now = UINT32_MAX - 2999;
  setup();
  at(0);
}

void protocolChecks() {
  bondedBoot();
  assert(Serial.baud == 115200 && advertising.active);
  assert(service.started &&
         service.uuid == "7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e");
  assert(characteristic.uuid == "7d2ea28b-f7bd-485a-bd9d-92ad6ecfe93e");
  assert(brightnessCharacteristic.uuid ==
         "7d2ea28c-f7bd-485a-bd9d-92ad6ecfe93e");
  for (auto* target : {&characteristic, &brightnessCharacteristic}) {
    assert(target->properties & BLECharacteristic::PROPERTY_WRITE);
    assert(target->permissions & ESP_GATT_PERM_WRITE_ENCRYPTED);
    assert(!target->write("R", false));
  }
  assert(strictSecurity && BLESecurity::auth == ESP_LE_AUTH_REQ_SC_BOND);
  assert(BLESecurity::capability == ESP_IO_CAP_NONE &&
         BLESecurity::keySize == 16);
  assert((BLESecurity::initKey & 3) == 3 && (BLESecurity::respKey & 3) == 3);
  assert(advertising.advertisement.uuid ==
         "7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e");
  assert(advertising.advertisement.manufacturer ==
         std::string("\xff\xff\x14\x63\x93\x92\x45\x24", 8));
  expectLamp(0, 0, 0, 0);
  feed("G");
  expectLamp(0, 0, 0, 0);
  feed("\r\n");
  expectLamp(0, 0, 255);
  now = 2000;
  feed("Y\n");
  expectLamp(0, 255, 0);
  now = 9000;
  feed("\nRG\n R\nR \nR\r\r\nR\rG\nr\n");
  feed(std::string("R\0", 2) + "\n");
  feed(std::string(1024, 'R'));
  feed("G\n");
  expectLamp(0, 255, 0);
  checkAt(12000, 0, 0, 0, 0);  // Invalid serial input cannot extend status.

  connectPeer();
  statusAt(13000, "N");
  checkAt(13250, 0, 0, 0, 0);
  statusAt(14000, "G");
  expectLamp(0, 0, 255);
  now = 22000;
  // An empty prepared-write commit must not replay the preceding valid value.
  characteristic.callbacks->onWrite(&characteristic);
  for (const auto& invalid :
       {std::string(), std::string("RY"), std::string("R\n"), std::string("r"),
        std::string("?"), std::string("R\0", 2)}) {
    characteristic.write(invalid);
    loop();
  }
  feed("R\nY\nN\n");
  expectLamp(0, 0, 255);  // BLE owns colors until disconnect.
  checkAt(23999, 0, 0, 255, 0);
  checkAt(24000, 0, 0, 0, 0);
  const uint32_t first = UINT32_MAX - 5000;
  statusAt(first, "G");
  statusAt(first + 2000,
           "G");  // Same-byte heartbeat extends the deadline across wrap.
  at(first + 10000);
  expectLamp(0, 0, 255);
  at(first + 11999);
  expectLamp(0, 0, 255);
  checkAt(first + 12000, 0, 0, 0, 0);
  disconnectPeer();
  loop();
  assert(advertising.active);
  const auto serialTime = now;
  feed("R\nG");
  expectLamp(255, 0, 0);
  connectPeer();
  disconnectPeer();
  loop();
  feed("\n");
  expectLamp(255, 0,
             0);  // Partial pre-BLE input cannot replay after disconnect.
  now = serialTime + 10000;
  Serial.input.push_back('x');
  Serial.flood = true;
  loop();
  expectLamp(0, 0, 0, 0);
  connectPeer();
  statusAt(now, "G");
  at(now + 250);
  expectLamp(0, 0, 255);  // USB flood cannot starve BLE/expiry.
}

void pairingChecks() {
  buttonLevel = LOW;
  setup();
  assert(BLEDevice::security->onSecurityRequest());
  now = 60000;
  assert(!BLEDevice::security->onSecurityRequest());
  at(80000);
  assert(!BLEDevice::security
              ->onSecurityRequest());  // Held at boot needs release.
  const std::string reply =
      "clawlight_pair identity=146393924524 "
      "service=7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e\n";
  Serial.output.clear();
  feed("INFO\r\n");
  assert(Serial.output.find(reply) != std::string::npos);
  feed("pair\nPAIR \n");
  feed(std::string("PAIR\0", 5) + "\n");
  assert(!BLEDevice::security->onSecurityRequest());
  mockBonds = {{{1, 2, 3, 4, 5, 6}}, {{2, 3, 4, 5, 6, 7}}};
  connectPeer();
  statusAt(81000, "G");
  at(81250);
  Serial.output.clear();
  feed("INFO\nPAIR\r\n");
  assert(Serial.output.find(reply) != std::string::npos);
  assert(BLEDevice::security->onSecurityRequest() && mockBonds.size() == 2);
  expectLamp(0, 0, 255);
  at(150000);
  assert(!BLEDevice::security->onSecurityRequest());
  buttonLevel = HIGH;
  loop();
  buttonLevel = LOW;
  const auto pressed = now;
  loop();
  at(pressed + 2999);
  assert(!BLEDevice::security->onSecurityRequest());
  at(pressed + 3000);
  assert(BLEDevice::security->onSecurityRequest());
  at(pressed + 9999);
  assert(mockBonds.size() == 2);
  at(pressed + 10000);
  assert(mockBonds.empty());
  assert(BLEDevice::security->onSecurityRequest());
  feed("R\n");
  expectLamp(255, 0, 0);  // Bond reset disconnects the old owner.
  buttonLevel = HIGH;
  loop();
  const uint32_t pairTime = UINT32_MAX - 30000;
  now = pairTime;
  feed("PAIR\n");
  now = pairTime + 59999;
  assert(BLEDevice::security->onSecurityRequest());
  ++now;
  assert(!BLEDevice::security->onSecurityRequest());
}

void indicationChecks() {
  constexpr int blue = BUILTIN_LED_BRIGHTNESS;
  setup();
  expectLamp(0, 0, 0, 0);
  checkAt(3000, 255, 0, 0, blue);
  checkAt(3100, 255, 0, 0, 0);
  checkAt(3250, 0, 255, 0, blue);
  checkAt(3500, 0, 0, 255, blue);
  checkAt(3750, 255, 0, 0, blue);
  checkAt(60000, 255, 255, 255, blue);
  checkAt(60999, 255, 255, 255, blue);
  checkAt(61000, 0, 0, 0, 0);
  checkAt(62000, 255, 255, 255, blue);
  mockBonds = {{{1, 2, 3, 4, 5, 6}}};
  checkAt(63000, 0, 0, 0, 0);
  // Waiting is dim and infrequent, without fixing its precise cadence.
  unsigned litSamples = 0;
  for (uint32_t elapsed = 0; elapsed < 90000; elapsed += 50) {
    at(63000 + elapsed);
    const auto duty = pinDuty(0);
    assert(duty <= 16 && pinDuty(1) == duty && pinDuty(3) == duty);
    if (elapsed < 10000) assert(duty == 0);
    if (duty > 0) ++litSamples;
  }
  assert(litSamples > 0 && litSamples <= 12);
  connectPeer();
  BLEDevice::security->onAuthenticationComplete({true, 0});
  checkAt(154000, 0, 0, 0, 0);  // Connection/encryption alone does not confirm.
  statusAt(155000, "N");
  expectLamp(255, 255, 255, blue);
  statusAt(155001, "N");
  expectLamp(255, 255, 255, blue);
  statusAt(155050, "G");
  expectLamp(255, 255, 255, blue);
  statusAt(155249, "R");
  expectLamp(255, 255, 255, blue);
  checkAt(155250, 255, 0, 0, blue);
  statusAt(157249, "R");
  expectLamp(255, 0, 0, 0);
  checkAt(158249, 255, 0, 0,
          blue);  // Heartbeats do not restart the builtin pattern.
  feed("PAIR\n");
  expectLamp(255, 0, 0);
  checkAt(167249, 0, 0, 0, 0);
  checkAt(197249, 0, 0, 0,
          0);  // Connected stale status stays dark, even in pairing mode.
  statusAt(197250, "N");
  expectLamp(0, 0, 0, 0);  // No repeated confirmation.
  disconnectPeer();
  loop();
  expectLamp(0, 0, 0, 0);
  for (unsigned pulses = 1; pulses <= 3; ++pulses) {
    const auto time = 200000 + pulses * 4000;
    now = time;
    feed(std::string(1, "GYR"[pulses - 1]) + "\n");
    const int r = pulses == 3 ? 255 : 0, y = pulses == 2 ? 255 : 0,
              g = pulses == 1 ? 255 : 0;
    expectLamp(r, y, g, blue);
    checkAt(time + 100, r, y, g, 0);
    checkAt(time + 250, r, y, g, pulses >= 2 ? blue : 0);
    checkAt(time + 500, r, y, g, pulses == 3 ? blue : 0);
    checkAt(time + 600, r, y, g, 0);
    checkAt(time + 3000, r, y, g, blue);
  }
  now = 220000;
  feed("N\nPAIR\n");
  expectLamp(0, 0, 0, 0);
  connectPeer();
  statusAt(221000, "N");
  expectLamp(255, 255, 255, blue);
  disconnectPeer();
  loop();
  expectLamp(0, 0, 0, 0);
  connectPeer();
  statusAt(222000, "N");
  BLEDevice::security->onAuthenticationComplete({false, 5});
  loop();
  expectLamp(0, 0, 0, 0);
  disconnectPeer();
  connectPeer();
  characteristic.write("G");
  disconnectPeer();
  connectPeer();
  loop();
  expectLamp(0, 0, 0, 0);
  now = 300000;
  characteristic.write("N");
  checkAt(309900, 255, 255, 255, blue);
  checkAt(310000, 0, 0, 0,
          0);  // A delayed first write still expires at receipt +10s.
}

// Queue hooks are only a stimulus seam for callbacks arriving inside loop().
void brightnessBeforeStatus(QueueHandle_t queue) {
  if (queue != bleStatuses) return;
  queueReceiveHook = nullptr;
  brightnessCharacteristic.write(std::string(1, '\x19'));
  characteristic.write("R");
}
void brightnessChecks() {
  bondedBoot();
  connectPeer();
  brightnessAt(100, 50);
  expectLamp(0, 0, 0, 0);
  for (const auto& bytes : {std::string(), std::string("50"),
                            std::string(1, '\x65'), std::string(1, '\xff')}) {
    brightnessCharacteristic.write(bytes);
    loop();
  }
  statusAt(1000, "N");
  expectLamp(128, 128, 128, BUILTIN_LED_BRIGHTNESS / 2);
  statusAt(1001, "G");
  at(1250);
  expectLamp(0, 0, 128);
  brightnessAt(3000, 0);
  expectLamp(0, 0, 0, 0);
  brightnessAt(4000, 100);
  expectLamp(0, 0, 255);
  brightnessAt(10000, 50);
  expectLamp(0, 0, 128);
  at(11000);
  expectLamp(0, 0, 128);
  checkAt(11001, 0, 0, 0, 0);  // Brightness never refreshes status freshness.
  disconnectPeer();
  connectPeer();
  statusAt(13000, "N");
  expectLamp(
      128, 128, 128,
      BUILTIN_LED_BRIGHTNESS / 2);  // Applied preference survives disconnect.
  brightnessCharacteristic.write(std::string(1, '\x4d'));
  disconnectPeer();
  connectPeer();
  statusAt(14000, "N");
  expectLamp(128, 128, 128,
             BUILTIN_LED_BRIGHTNESS / 2);  // Obsolete preference is rejected.
  disconnectPeer();
  connectPeer();
  now = 15000;
  queueReceiveHook = brightnessBeforeStatus;
  loop();
  expectLamp(64, 64, 64,
             BUILTIN_LED_BRIGHTNESS / 4);  // Preference precedes first status.
  disconnectPeer();
  loop();
  expectLamp(64, 0, 0);
  at(25000);
  expectLamp(0, 0, 0, 0);
  connectPeer();
  brightnessAt(26000, 0);
  statusAt(27000, "N");
  expectLamp(0, 0, 0, 0);
  disconnectPeer();
  loop();
  checkAt(37000, 0, 0, 0, 0);
  checkAt(67000, 0, 0, 0, 0);
  feed("PAIR\n");
  expectLamp(0, 0, 0, 0);
  mockBonds.clear();
  checkAt(128000, 0, 0, 0, 0);
}

void startupChecks(bool live) {
  mockBonds = {{{1, 2, 3, 4, 5, 6}}};
  setup();
  checkAt(0, 0, 0, 0, 0);
  if (live) {
    connectPeer();
    brightnessAt(100, 50);
    statusAt(200, "N");
    previewAt(300, 75);
    checkAt(750, 128, 128, 128, BUILTIN_LED_BRIGHTNESS / 2);
    checkAt(1500, 0, 0, 0, 0);
    checkAt(2250, 128, 128, 128, BUILTIN_LED_BRIGHTNESS / 2);
    statusAt(2900, "Y");
    checkAt(3000, 0, 128, 0,
            0);  // Latest status, no deferred connection flash.
    return;
  }
  now = 100;
  feed("G\n");  // Serial remains responsive throughout startup.
  for (uint32_t cycle : {0, 1500}) {
    at(cycle + 375);
    const auto rising = pinDuty(0);
    assert(rising > 0 && rising < 255);
    expectLamp(rising, rising, rising,
               std::lround(rising * BUILTIN_LED_BRIGHTNESS / 255));
    checkAt(cycle + 750, 255, 255, 255, BUILTIN_LED_BRIGHTNESS);
    at(cycle + 1125);
    const auto falling = pinDuty(0);
    assert(falling > 0 && falling < 255);
    expectLamp(falling, falling, falling,
               std::lround(falling * BUILTIN_LED_BRIGHTNESS / 255));
    at(cycle + 1499);
    assert(pinDuty(0) < falling && pinDuty(0) < 5);
    if (cycle == 0) checkAt(1500, 0, 0, 0, 0);
  }
  checkAt(3000, 0, 0, 255, 0);
}

void previewChecks() {
  bondedBoot();
  connectPeer();
  brightnessAt(100, 25);
  previewAt(150, 75);
  expectLamp(0, 0, 0, 0);  // A preview needs a fresh, confirmed connection.
  statusAt(200, "G");
  at(450);
  expectLamp(0, 0, 64);
  previewAt(1000, 75);
  expectLamp(191, 191, 191, BUILTIN_LED_BRIGHTNESS * 3 / 4);
  brightnessAt(1200, 50);
  expectLamp(191, 191, 191, BUILTIN_LED_BRIGHTNESS * 3 / 4);
  statusAt(1500, "R");
  expectLamp(191, 191, 191, BUILTIN_LED_BRIGHTNESS * 3 / 4);
  previewAt(2000, 75);  // Equal values still refresh the two-second preview.
  checkAt(3999, 191, 191, 191, BUILTIN_LED_BRIGHTNESS * 3 / 4);
  checkAt(4000, 128, 0, 0, 0);
  now = 4100;
  brightnessCharacteristic.write(std::string("\0\1", 2));
  brightnessAt(4100, 100);  // A queued saved setting must not erase a preview.
  expectLamp(0, 0, 0, 0);
  checkAt(6099, 0, 0, 0, 0);
  checkAt(6100, 255, 0, 0, 0);
  previewAt(6150, 100);
  expectLamp(255, 255, 255, BUILTIN_LED_BRIGHTNESS);
  statusAt(6200, "G");
  previewAt(6300, 50);
  now = 6400;
  assert(!brightnessCharacteristic.write(std::string("\x64\1", 2), false));
  for (const auto& bytes :
       {std::string(), std::string("\x64\0", 2), std::string("\x64\3", 2),
        std::string("\x65\1", 2), std::string("\xff\1", 2),
        std::string("\x64\1\1", 3)}) {
    brightnessCharacteristic.write(bytes);
    loop();
  }
  checkAt(8299, 128, 128, 128, BUILTIN_LED_BRIGHTNESS / 2);
  checkAt(8300, 0, 0, 255, 0);
  previewAt(16150, 50);
  checkAt(16199, 128, 128, 128, BUILTIN_LED_BRIGHTNESS / 2);
  checkAt(16200, 0, 0, 0, 0);  // Preview never extends the status deadline.
  previewAt(17000, 50);
  expectLamp(0, 0, 0, 0);
  statusAt(18000, "N");
  previewAt(18100, 50);
  expectLamp(128, 128, 128, BUILTIN_LED_BRIGHTNESS / 2);
  disconnectPeer();
  loop();
  expectLamp(0, 0, 0, 0);
  connectPeer();
  statusAt(18500, "G");
  at(18750);
  expectLamp(0, 0, 255);  // The preceding connection's preview cannot reappear.
  now = 19000;
  brightnessCharacteristic.write(std::string("\x4b\1", 2));
  disconnectPeer();
  connectPeer();
  statusAt(19001, "G");
  expectLamp(255, 255, 255, BUILTIN_LED_BRIGHTNESS);
  checkAt(19251, 0, 0, 255, 0);
  now = 19500;
  brightnessCharacteristic.write(std::string("\x19\1", 2));
  checkAt(21500, 0, 0, 255,
          -1);  // Expiry starts at receipt, not loop delivery.
  const uint32_t receipt = UINT32_MAX - 500;
  statusAt(receipt - 500, "G");
  previewAt(receipt, 25);
  checkAt(receipt + 1999, 64, 64, 64, BUILTIN_LED_BRIGHTNESS / 4);
  checkAt(receipt + 2000, 0, 0, 255, -1);
  checkAt(receipt + 9500, 0, 0, 0, 0);
}

void identifyChecks() {
  bondedBoot();
  connectPeer();
  brightnessAt(100, 0);
  previewAt(150, 100, 2);
  expectLamp(0, 0, 0, 0);  // Identify also requires a confirmed connection.
  statusAt(200, "G");
  previewAt(1000, 100, 2);
  for (uint32_t step = 0; step < 9; ++step) {
    const uint32_t time = 1000 + step * 250;
    const int red = step % 3 == 0 ? 255 : 0;
    const int yellow = step % 3 == 1 ? 255 : 0;
    const int green = step % 3 == 2 ? 255 : 0;
    checkAt(time, red, yellow, green, BUILTIN_LED_BRIGHTNESS);
    if (step == 3) {
      statusAt(time + 100, "R");
      brightnessAt(time + 150, 50);
    }
    checkAt(time + 249, red, yellow, green, 0);
  }
  checkAt(3250, 128, 0, 0,
          0);  // Restore the latest status and saved brightness.
  previewAt(4000, 100, 2);
  checkAt(4499, 0, 255, 0, 0);
  previewAt(4500, 100, 2);  // Repeated requests restart at red.
  expectLamp(255, 0, 0, BUILTIN_LED_BRIGHTNESS);
  now = 4600;
  assert(!brightnessCharacteristic.write(std::string("\x64\2", 2), false));
  for (const auto& bytes :
       {std::string(), std::string("\x64\0", 2), std::string("\x64\3", 2),
        std::string("\x65\2", 2), std::string("\xff\2", 2),
        std::string("\x64\2\2", 3)}) {
    brightnessCharacteristic.write(bytes);
    loop();
  }
  checkAt(4750, 0, 255, 0, BUILTIN_LED_BRIGHTNESS);
  checkAt(6749, 0, 0, 255, 0);
  checkAt(6750, 128, 0, 0, 0);
  previewAt(7000, 100, 2);
  disconnectPeer();
  loop();
  expectLamp(128, 0, 0);  // Disconnect cancels the scan immediately.
  connectPeer();
  statusAt(7200, "N");
  checkAt(7450, 0, 0, 0, 0);
  now = 7500;
  brightnessCharacteristic.write(std::string("\x64\2", 2));
  disconnectPeer();
  connectPeer();
  statusAt(7501, "G");
  checkAt(7751, 0, 0, 128, 0);  // An obsolete queued scan cannot reappear.
  statusAt(8000, "G");
  previewAt(8100, 100, 2);
  previewAt(8200, 25);
  checkAt(10199, 64, 64, 64, BUILTIN_LED_BRIGHTNESS / 4);
  checkAt(10200, 0, 0, 128, 0);  // A brightness preview replaces the scan.
  now = 12000;
  brightnessCharacteristic.write(std::string("\x64\2", 2));
  checkAt(14250, 0, 0, 128, -1);  // Scan expiry starts at receipt.
  previewAt(17750, 100, 2);
  checkAt(17999, 255, 0, 0, 0);
  checkAt(18000, 0, 0, 0, 0);  // Identify cannot extend status freshness.
  previewAt(18100, 100, 2);
  expectLamp(0, 0, 0, 0);
  statusAt(19000, "N");
  previewAt(19100, 100, 2);
  BLEDevice::security->onAuthenticationComplete({false, 5});
  loop();
  expectLamp(0, 0, 0, 0);
  const uint32_t receipt = UINT32_MAX - 500;
  statusAt(receipt - 500, "G");
  previewAt(receipt, 25, 2);
  checkAt(receipt + 2249, 0, 0, 64, 0);
  checkAt(receipt + 2250, 0, 0, 128, -1);
}

void connectDuringStatusRead(QueueHandle_t queue) {
  if (queue != bleStatuses) return;
  queueReceiveHook = nullptr;
  connectPeer();
  characteristic.write("G");
}
char disconnectStatus = 'R';
void disconnectDuringStatusRead(QueueHandle_t queue) {
  if (queue != bleStatuses) return;
  queueReceiveHook = nullptr;
  characteristic.write(std::string(1, disconnectStatus));
  disconnectPeer();
}
void connectBeforeSerialRead() {
  serialAvailableHook = nullptr;
  connectPeer();
}
void callbackRaceChecks() {
  bondedBoot();
  Serial.input = {'R', '\n'};
  now = 1000;
  queueReceiveHook = connectDuringStatusRead;
  loop();
  at(1250);
  expectLamp(0, 0, 255);
  queueReceiveHook = disconnectDuringStatusRead;
  loop();
  expectLamp(255, 0,
             0);  // Acknowledged same-connection color survives disconnect.
  connectPeer();
  statusAt(2000, "G");
  disconnectStatus = 'N';
  queueReceiveHook = disconnectDuringStatusRead;
  loop();
  expectLamp(0, 0, 0, 0);  // Acknowledged shutdown also survives disconnect.
  Serial.input = {'R', '\n'};
  serialAvailableHook = connectBeforeSerialRead;
  loop();
  expectLamp(0, 0, 0, 0);  // Live BLE ownership already blocks USB.
}

void serviceChangeChecks(bool modeFailure) {
  mockChangeModeResult = modeFailure ? 255 : 0;
  bondedBoot();
  esp_ble_gatts_cb_param_t parameters{};
  const uint8_t address[] = {0xaa, 0x12, 0x34, 0x56, 0x78, 0xbb};
  std::memcpy(parameters.connect.remote_bda, address, sizeof(address));
  assert(BLEDevice::gattsHandler != nullptr);
  BLEDevice::gattsHandler(ESP_GATTS_CONNECT_EVT, 7, &parameters);
  if (modeFailure)
    assert(mockChangeRequests == 0);
  else {
    assert(mockChangeRequests > 0 && mockChangeInterface == 7);
    assert(std::memcmp(mockChangeAddress, address, sizeof(address)) == 0);
  }
  assert(mockBonds.size() == 1 && !BLEDevice::security->onSecurityRequest());
}
int main(int argc, char** argv) {
  assert(argc == 2);
  const std::string scenario = argv[1];
  if (scenario == "protocol")
    protocolChecks();
  else if (scenario == "pairing")
    pairingChecks();
  else if (scenario == "indications")
    indicationChecks();
  else if (scenario == "brightness")
    brightnessChecks();
  else if (scenario == "startup")
    startupChecks(false);
  else if (scenario == "startup-live")
    startupChecks(true);
  else if (scenario == "preview")
    previewChecks();
  else if (scenario == "identify")
    identifyChecks();
  else if (scenario == "races")
    callbackRaceChecks();
  else if (scenario == "gatt-change")
    serviceChangeChecks(false);
  else if (scenario == "gatt-mode-failed")
    serviceChangeChecks(true);
  else
    assert(false && "Unknown scenario");
}
