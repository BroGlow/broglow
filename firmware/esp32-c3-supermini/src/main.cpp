#include <Arduino.h>
#include <BLEDevice.h>
#include <BLESecurity.h>
#include <BLEServer.h>
#include <esp_gatts_api.h>
#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>

#include <algorithm>
#include <atomic>
#include <cstring>
#include <vector>

// Pinned IDF 4.4.7 exports this BTA API but omits it from its public headers.
// Its prebuilt AUTO mode rejects manual indications; runtime MANUAL (1) allows
// bonded Macs to invalidate a GATT cache retained across firmware upgrades.
// https://github.com/espressif/esp-idf/blob/v4.4.7/components/bt/host/bluedroid/bta/gatt/bta_gatts_api.c#L597
extern "C" uint8_t BTA_GATTS_SetServiceChangeMode(uint8_t mode);

constexpr char SERVICE_UUID[] = "7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e";
constexpr char STATUS_UUID[] = "7d2ea28b-f7bd-485a-bd9d-92ad6ecfe93e";
constexpr char BRIGHTNESS_UUID[] = "7d2ea28c-f7bd-485a-bd9d-92ad6ecfe93e";
constexpr uint32_t PAIRING_MS = 60000;
constexpr uint32_t IDENTIFY_STEP_MS = 100;
constexpr int BUTTON_PIN =
    9;  // BOOT: use only after normal boot, never hold during reset.

constexpr bool safePin(int pin) {
  return pin >= 0 && pin <= 10 && pin != 2 && pin != 8 && pin != 9;
}
static_assert(safePin(LED_RED_PIN) && safePin(LED_YELLOW_PIN) &&
                  safePin(LED_GREEN_PIN),
              "Choose exposed non-strapping GPIOs (defaults: 0, 1, 3)");
static_assert(LED_RED_PIN != LED_YELLOW_PIN && LED_RED_PIN != LED_GREEN_PIN &&
                  LED_YELLOW_PIN != LED_GREEN_PIN,
              "Each LED needs its own GPIO");
static_assert(STATUS_TIMEOUT_MS > 2000,
              "Timeout must exceed the CLI's 2-second heartbeat");
static_assert(BUILTIN_LED_PIN == -1 ||
                  ((safePin(BUILTIN_LED_PIN) || BUILTIN_LED_PIN == 8) &&
                   BUILTIN_LED_PIN != LED_RED_PIN &&
                   BUILTIN_LED_PIN != LED_YELLOW_PIN &&
                   BUILTIN_LED_PIN != LED_GREEN_PIN),
              "Builtin LED needs its own GPIO, or -1 to disable");
static_assert(BUILTIN_LED_BRIGHTNESS >= 0 && BUILTIN_LED_BRIGHTNESS <= 255 &&
                  WAIT_LED_BRIGHTNESS >= 0 && WAIT_LED_BRIGHTNESS <= 255,
              "Brightness range is 0..255");
static_assert(BUILTIN_LED_ACTIVE_LOW == 0 || BUILTIN_LED_ACTIVE_LOW == 1,
              "Polarity must be 0 or 1");

char line[64];
size_t used = 0;
bool overflow = false;
uint32_t lastStatus = 0;
bool lit = false;
bool hasStatus = false;
bool bleConnected = false;
std::atomic<bool> bleLinkConnected{false};
char appliedStatus = 'N';
uint32_t acceptedStatuses = 0;
char identity[13];
BLEServer* bleServer;
std::atomic<bool> pairingEnabled{false};
std::atomic<uint32_t> pairingStarted{0};
std::atomic<bool> authenticated{false};
std::atomic<int> authenticationResult{-1};
std::atomic<uint32_t> bleGeneration{0};
std::atomic<bool> serviceChangeManual{false};
std::atomic<uint32_t> serviceChangeRequests{0};
std::atomic<int> serviceChangeRequestResult{-1}, serviceChangeResult{-1};
uint32_t confirmedGeneration = 0;
bool connectionPulse = false;
uint32_t connectionPulseStarted = 0, statusPatternStarted = 0,
         indicationStarted = 0;
bool startup = true, preview = false, previewIdentify = false;
uint32_t startupStarted = 0, previewStarted = 0, previewGeneration = 0;
uint8_t previewPercent = 0;
enum class Indication {
  Startup,
  Preview,
  Identify,
  Status,
  Pairing,
  Unpaired,
  Waiting,
  Connected,
  Stale
};
Indication indication = Indication::Waiting;
uint8_t duties[4] = {};
uint8_t brightness = 100;
bool buttonReady = false, buttonDown = false, openedOnHold = false,
     resetOnHold = false;
uint32_t buttonPressed = 0;

bool pairingOpen() {
  return pairingEnabled.load() &&
         static_cast<uint32_t>(millis() - pairingStarted.load()) < PAIRING_MS;
}

void openPairing() {
  pairingStarted.store(millis());
  pairingEnabled.store(true);
  Serial.print("pairing=open duration_ms=60000\n");
}

void resetBonds() {
  if (bleConnected) bleServer->disconnect(bleServer->getConnId());
  int count = esp_ble_get_bond_device_num();
  if (count < 0) {
    Serial.print("bond_reset=error\n");
    return;
  }
  std::vector<esp_ble_bond_dev_t> bonds(count);
  if (count && esp_ble_get_bond_device_list(&count, bonds.data()) != ESP_OK) {
    Serial.print("bond_reset=error\n");
    return;
  }
  for (int i = 0; i < count; ++i) {
    if (esp_ble_remove_bond_device(bonds[i].bd_addr) != ESP_OK) {
      Serial.print("bond_reset=error\n");
      return;
    }
  }
  Serial.printf("bond_reset=requested count=%d\n", count);
  openPairing();
}

void checkButton() {
  if (digitalRead(BUTTON_PIN) != LOW) {
    buttonReady = true;
    buttonDown = openedOnHold = resetOnHold = false;
    return;
  }
  if (!buttonReady)
    return;  // Require a release after boot before recognizing holds.
  if (!buttonDown) {
    buttonDown = true;
    buttonPressed = millis();
  }
  const uint32_t held = millis() - buttonPressed;
  if (!openedOnHold && held >= 3000) {
    openedOnHold = true;
    openPairing();
  }
  if (!resetOnHold && held >= 10000) {
    resetOnHold = true;
    resetBonds();
  }
}

class SecurityCallbacks : public BLESecurityCallbacks {
  uint32_t onPassKeyRequest() override { return 0; }
  void onPassKeyNotify(uint32_t) override {}
  bool onConfirmPIN(uint32_t) override {
    return false;
  }  // No display/keypad: Just Works.
  bool onSecurityRequest() override { return pairingOpen(); }
  void onAuthenticationComplete(esp_ble_auth_cmpl_t result) override {
    authenticated.store(result.success);
    authenticationResult.store(result.success ? 0 : 0x100 | result.fail_reason);
  }
} securityCallbacks;

// Latest state wins; callbacks never touch GPIO or the loop's timeout state.
struct BleStatus {
  char state;
  uint32_t received;
  uint32_t generation;
};
struct BleBrightness {
  uint8_t percent;
  uint32_t generation;
  uint32_t received;
  bool identify;
};
QueueHandle_t bleConnections;
QueueHandle_t bleStatuses;
QueueHandle_t bleBrightness;
QueueHandle_t blePreviews;

bool validStatus(char state) {
  return state == 'R' || state == 'Y' || state == 'G' || state == 'N';
}

class ServerCallbacks : public BLEServerCallbacks {
  void onConnect(BLEServer*) override {
    ++bleGeneration;
    authenticated.store(false);
    bleLinkConnected.store(true);
    const bool connected = true;
    xQueueOverwrite(bleConnections, &connected);
  }
  void onDisconnect(BLEServer*) override {
    bleLinkConnected.store(false);
    authenticated.store(false);
    const bool connected = false;
    xQueueOverwrite(bleConnections, &connected);
  }
} serverCallbacks;

void gattEvents(esp_gatts_cb_event_t event, esp_gatt_if_t interface,
                esp_ble_gatts_cb_param_t* parameters) {
  if (event == ESP_GATTS_CONNECT_EVT && serviceChangeManual.load()) {
    serviceChangeResult.store(-1);
    ++serviceChangeRequests;
    // ESP_OK means queued; SEND_SERVICE_CHANGE_EVT reports the stack result,
    // not the central's confirmation or successful rediscovery.
    serviceChangeRequestResult.store(
        esp_ble_gatts_send_service_change_indication(
            interface, parameters->connect.remote_bda));
  } else if (event == ESP_GATTS_SEND_SERVICE_CHANGE_EVT) {
    serviceChangeResult.store(parameters->service_change.status);
  }
}

class StatusCallbacks : public BLECharacteristicCallbacks {
  void onWrite(BLECharacteristic* characteristic) override {
    const std::string value = characteristic->getValue();
    // Arduino's empty prepared-write commit retains the old value. Never replay
    // it.
    characteristic->setValue(std::string());
    if (value.size() != 1 || !validStatus(value[0])) return;
    const BleStatus status{value[0], millis(), bleGeneration.load()};
    xQueueOverwrite(bleStatuses, &status);
  }
} statusCallbacks;

class BrightnessCallbacks : public BLECharacteristicCallbacks {
  void onWrite(BLECharacteristic* characteristic) override {
    const std::string value = characteristic->getValue();
    characteristic->setValue(
        std::string());  // Empty prepared writes must not replay a value.
    if ((value.size() != 1 &&
         (value.size() != 2 || (value[1] != 1 && value[1] != 2))) ||
        static_cast<uint8_t>(value[0]) > 100)
      return;
    const BleBrightness setting{static_cast<uint8_t>(value[0]),
                                bleGeneration.load(), millis(),
                                value.size() == 2 && value[1] == 2};
    xQueueOverwrite(value.size() == 1 ? bleBrightness : blePreviews, &setting);
  }
} brightnessCallbacks;

void applyPendingBrightness() {
  BleBrightness setting;
  if (xQueueReceive(bleBrightness, &setting, 0) == pdTRUE &&
      setting.generation == bleGeneration.load()) {
    brightness = setting.percent;
  }
  if (xQueueReceive(blePreviews, &setting, 0) == pdTRUE &&
      setting.generation == bleGeneration.load() && bleLinkConnected.load()) {
    preview = true;
    previewIdentify = setting.identify;
    previewPercent = setting.percent;
    previewStarted = setting.received;
    previewGeneration = setting.generation;
  }
}

void showStatus(char state) {
  lit = state != 'N';
  appliedStatus = state;
}

void applyStatus(char state, uint32_t received) {
  if (!hasStatus || state != appliedStatus) statusPatternStarted = received;
  showStatus(state);
  hasStatus = true;
  lastStatus = received;
  ++acceptedStatuses;
}

void outputLeds(uint8_t red, uint8_t yellow, uint8_t green, uint8_t builtin,
                uint8_t percent) {
  uint8_t next[] = {red, yellow, green,
                    BUILTIN_LED_PIN < 0 ? uint8_t(0) : builtin};
  for (size_t channel = 0; channel < 4; ++channel) {
    next[channel] = (static_cast<unsigned>(next[channel]) * percent + 50) / 100;
    if (next[channel] == duties[channel]) continue;
    duties[channel] = next[channel];
    ledcWrite(channel, channel == 3 && BUILTIN_LED_ACTIVE_LOW
                           ? 255 - next[channel]
                           : next[channel]);
  }
}

void renderIndication() {
  const uint32_t now = millis();
  if (hasStatus &&
      static_cast<uint32_t>(now - lastStatus) >= STATUS_TIMEOUT_MS) {
    hasStatus = false;
    showStatus('N');
  }
  if (connectionPulse &&
      (!hasStatus || !bleLinkConnected.load() || !authenticated.load() ||
       static_cast<uint32_t>(now - connectionPulseStarted) >= 250)) {
    connectionPulse = false;
  }
  if (startup && static_cast<uint32_t>(now - startupStarted) >= 3000)
    startup = false;
  if (preview &&
      (!hasStatus || !bleLinkConnected.load() || !authenticated.load() ||
       previewGeneration != bleGeneration.load() ||
       confirmedGeneration != previewGeneration ||
       static_cast<uint32_t>(now - previewStarted) >=
           (previewIdentify ? 9 * IDENTIFY_STEP_MS : 2000)))
    preview = false;
  const Indication next =
      startup                      ? Indication::Startup
      : preview && previewIdentify ? Indication::Identify
      : preview                    ? Indication::Preview
      : connectionPulse            ? Indication::Connected
      : hasStatus                  ? Indication::Status
      : bleConnected && confirmedGeneration == bleGeneration.load()
          ? Indication::Stale
      : pairingOpen()                      ? Indication::Pairing
      : esp_ble_get_bond_device_num() == 0 ? Indication::Unpaired
      : bleConnected                       ? Indication::Stale
                                           : Indication::Waiting;
  if (next != indication) {
    indication = next;
    indicationStarted = now;
  }
  uint8_t red = 0, yellow = 0, green = 0, builtin = 0;
  switch (indication) {
    case Indication::Startup: {
      const uint32_t phase = (now - startupStarted) % 1500;
      const uint32_t ramp = phase <= 750 ? phase : 1500 - phase;
      red = yellow = green = ramp * 255 / 750;
      builtin = ramp * BUILTIN_LED_BRIGHTNESS / 750;
      break;
    }
    case Indication::Preview:
    case Indication::Connected:
      red = yellow = green = 255;
      builtin = BUILTIN_LED_BRIGHTNESS;
      break;
    case Indication::Status: {
      red = appliedStatus == 'R' ? 255 : 0;
      yellow = appliedStatus == 'Y' ? 255 : 0;
      green = appliedStatus == 'G' ? 255 : 0;
      const unsigned pulses = appliedStatus == 'R'   ? 3
                              : appliedStatus == 'Y' ? 2
                              : appliedStatus == 'G' ? 1
                                                     : 0;
      const uint32_t phase = (now - statusPatternStarted) % 3000;
      if (phase / 250 < pulses && phase % 250 < 100)
        builtin = BUILTIN_LED_BRIGHTNESS;
      break;
    }
    case Indication::Identify:
    case Indication::Pairing: {
      const uint32_t phase =
          now - (indication == Indication::Identify ? previewStarted
                                                    : pairingStarted.load());
      const uint32_t step =
          indication == Indication::Identify ? IDENTIFY_STEP_MS : 250;
      red = phase / step % 3 == 0 ? 255 : 0;
      yellow = phase / step % 3 == 1 ? 255 : 0;
      green = phase / step % 3 == 2 ? 255 : 0;
      if (phase % step < step * 2 / 5) builtin = BUILTIN_LED_BRIGHTNESS;
      break;
    }
    case Indication::Unpaired:
      if ((now - indicationStarted) / 1000 % 2 == 0) {
        red = yellow = green = 255;
        builtin = BUILTIN_LED_BRIGHTNESS;
      }
      break;
    case Indication::Waiting: {
      const uint32_t elapsed = now - indicationStarted;
      if (elapsed >= 30000 && elapsed % 30000 < 100) {
        red = yellow = green = builtin = WAIT_LED_BRIGHTNESS;
      }
      break;
    }
    case Indication::Stale:
      break;
  }
  outputLeds(
      red, yellow, green, builtin,
      indication == Indication::Preview || indication == Indication::Identify
          ? previewPercent
          : brightness);
}

const char* indicationName() {
  switch (indication) {
    case Indication::Startup:
      return "startup";
    case Indication::Preview:
      return "preview";
    case Indication::Identify:
      return "identify";
    case Indication::Status:
      return "status";
    case Indication::Pairing:
      return "pairing";
    case Indication::Unpaired:
      return "unpaired";
    case Indication::Waiting:
      return "waiting";
    case Indication::Connected:
      return "connected";
    case Indication::Stale:
      return "stale";
  }
  return "stale";
}

void diagnostic() {
  renderIndication();
  Serial.printf(
      "state=%c red=%d yellow=%d green=%d owner=%s connected=%d "
      "authenticated=%d pairing=%d bonds=%d age_ms=%lu accepted=%lu mode=%s "
      "red_pwm=%u yellow_pwm=%u green_pwm=%u builtin_pwm=%u builtin_pin=%d "
      "brightness=%u gatt_change_manual=%d gatt_change_requests=%lu "
      "gatt_change_request=%d gatt_change_result=%d\n",
      appliedStatus, duties[0] > 0, duties[1] > 0, duties[2] > 0,
      bleConnected ? "ble" : "serial", bleConnected,
      bleConnected && authenticated.load(), pairingOpen(),
      esp_ble_get_bond_device_num(),
      static_cast<unsigned long>(millis() - lastStatus),
      static_cast<unsigned long>(acceptedStatuses), indicationName(), duties[0],
      duties[1], duties[2], duties[3], BUILTIN_LED_PIN, brightness,
      serviceChangeManual.load(),
      static_cast<unsigned long>(serviceChangeRequests.load()),
      serviceChangeRequestResult.load(), serviceChangeResult.load());
}

void reportIdentity() {
  Serial.printf("clawlight_pair identity=%s service=%s\n", identity,
                SERVICE_UUID);
}

void setup() {
  Serial.begin(115200);
  const int pins[] = {LED_RED_PIN, LED_YELLOW_PIN, LED_GREEN_PIN,
                      BUILTIN_LED_PIN};
  for (size_t channel = 0; channel < 4; ++channel) {
    if (pins[channel] < 0) continue;
    digitalWrite(pins[channel],
                 channel == 3 && BUILTIN_LED_ACTIVE_LOW ? HIGH : LOW);
    pinMode(pins[channel], OUTPUT);
    configASSERT(ledcSetup(channel, 2000, 8) != 0);
    // Set idle duty before attaching: active-low onboard LED must start off.
    ledcWrite(channel, channel == 3 && BUILTIN_LED_ACTIVE_LOW ? 255 : 0);
    ledcAttachPin(pins[channel], channel);
  }
  pinMode(BUTTON_PIN, INPUT_PULLUP);
  showStatus('N');
  bleConnections = xQueueCreate(1, sizeof(bool));
  bleStatuses = xQueueCreate(1, sizeof(BleStatus));
  bleBrightness = xQueueCreate(1, sizeof(BleBrightness));
  blePreviews = xQueueCreate(1, sizeof(BleBrightness));
  configASSERT(bleConnections && bleStatuses && bleBrightness && blePreviews);
  const uint64_t mac = ESP.getEfuseMac();
  char identityBytes[6];
  for (size_t i = 0; i < sizeof(identityBytes); ++i) {
    const auto byte = static_cast<unsigned>((mac >> (8 * i)) & 0xff);
    identityBytes[i] = static_cast<char>(byte);
    snprintf(identity + 2 * i, 3, "%02x", byte);
  }
  char name[24];
  snprintf(name, sizeof(name), "ClawLight C3 %02X%02X",
           static_cast<unsigned>((mac >> 32) & 0xff),
           static_cast<unsigned>((mac >> 40) & 0xff));
  BLEDevice::init(name);
  const uint8_t changeModeResult = BTA_GATTS_SetServiceChangeMode(1);
  serviceChangeManual.store(changeModeResult == 0);
  Serial.printf("gatt_change_manual=%d result=%u\n", serviceChangeManual.load(),
                changeModeResult);
  BLEDevice::setCustomGattsHandler(gattEvents);
  BLEDevice::setSecurityCallbacks(&securityCallbacks);
  BLESecurity security;
  security.setAuthenticationMode(ESP_LE_AUTH_REQ_SC_BOND);
  security.setCapability(ESP_IO_CAP_NONE);
  security.setKeySize(16);
  security.setInitEncryptionKey(ESP_BLE_ENC_KEY_MASK | ESP_BLE_ID_KEY_MASK);
  security.setRespEncryptionKey(ESP_BLE_ENC_KEY_MASK | ESP_BLE_ID_KEY_MASK);
  uint8_t strict = ESP_BLE_ONLY_ACCEPT_SPECIFIED_AUTH_ENABLE;
  const auto securityResult = esp_ble_gap_set_security_param(
      ESP_BLE_SM_ONLY_ACCEPT_SPECIFIED_SEC_AUTH, &strict, sizeof(strict));
  configASSERT(securityResult == ESP_OK);
  // Do not setEncryptionLevel(): that initiates pairing before our
  // physical-window gate.
  bleServer = BLEDevice::createServer();
  bleServer->setCallbacks(&serverCallbacks);
  BLEService* service = bleServer->createService(SERVICE_UUID);
  BLECharacteristic* status = service->createCharacteristic(
      STATUS_UUID, BLECharacteristic::PROPERTY_WRITE);
  status->setCallbacks(&statusCallbacks);
  status->setAccessPermissions(ESP_GATT_PERM_WRITE_ENCRYPTED);
  BLECharacteristic* brightnessSetting = service->createCharacteristic(
      BRIGHTNESS_UUID, BLECharacteristic::PROPERTY_WRITE);
  brightnessSetting->setCallbacks(&brightnessCallbacks);
  brightnessSetting->setAccessPermissions(ESP_GATT_PERM_WRITE_ENCRYPTED);
  service->start();
  BLEAdvertising* advertising = BLEDevice::getAdvertising();
  BLEAdvertisementData advertisement, scanResponse;
  advertisement.setFlags(ESP_BLE_ADV_FLAG_GEN_DISC |
                         ESP_BLE_ADV_FLAG_BREDR_NOT_SPT);
  advertisement.setCompleteServices(BLEUUID(SERVICE_UUID));
  // 3 flag + 18 service + 10 manufacturer bytes fill the legacy 31-byte packet.
  // Company 0xffff is for local prototypes; identify by service UUID AND full
  // MAC.
  advertisement.setManufacturerData(
      std::string(2, '\xff') +
      std::string(identityBytes, sizeof(identityBytes)));
  configASSERT(advertisement.getPayload().size() == 31);
  scanResponse.setName(name);
  advertising->setAdvertisementData(advertisement);
  advertising->setScanResponseData(scanResponse);
  advertising->start();
  if (esp_ble_get_bond_device_num() == 0) openPairing();
  indicationStarted = millis();
  startupStarted = millis();
  Serial.printf("lamp=%s security=sc-bonded button=BOOT\n", name);
}

void synchronizeBleConnection() {
  if (xQueueReceive(bleConnections, &bleConnected, 0) == pdTRUE) {
    connectionPulse = false;
    preview = false;
    used = 0;
    overflow = false;
    if (!bleConnected) BLEDevice::startAdvertising();
  }
}

void loop() {
  checkButton();
  if (pairingEnabled.load() && !pairingOpen()) {
    pairingEnabled.store(false);
    Serial.print("pairing=closed\n");
  }
  const int auth = authenticationResult.exchange(-1);
  if (auth >= 0)
    Serial.printf("authentication=%s reason=%d\n", auth ? "failed" : "ok",
                  auth & 0xff);
  synchronizeBleConnection();
  applyPendingBrightness();
  BleStatus status;
  if (xQueueReceive(bleStatuses, &status, 0) == pdTRUE) {
    // Callbacks can arrive between the two queue reads. Synchronize before
    // applying.
    synchronizeBleConnection();
    applyPendingBrightness();  // A brightness write acknowledged before this
                               // status wins.
    const bool connected = bleConnected && bleLinkConnected.load();
    // Preserve already acknowledged state after disconnect, using its original
    // timestamp. Only an obsolete connection generation is rejected; a
    // disconnected write cannot confirm.
    if (status.generation == bleGeneration.load()) {
      // The GATT permission rejects unencrypted writes before the callback
      // runs.
      if (connected) authenticated.store(true);
      applyStatus(status.state, status.received);
      if (connected &&
          static_cast<uint32_t>(millis() - status.received) <
              STATUS_TIMEOUT_MS &&
          confirmedGeneration != status.generation) {
        confirmedGeneration = status.generation;
        // Startup already confirms power; do not replay a connection flash
        // for a status received during its two breathing cycles.
        connectionPulse =
            !startup ||
            (static_cast<uint32_t>(millis() - startupStarted) >= 3000 &&
             static_cast<uint32_t>(status.received - startupStarted) >= 3000);
        connectionPulseStarted = millis();
      }
    }
  }
  // Bound serial work so continuous malformed USB input cannot starve
  // BLE/expiry.
  const size_t serialBudget =
      std::min(static_cast<size_t>(Serial.available()), sizeof(line));
  for (size_t i = 0; i < serialBudget; ++i) {
    const char ch = static_cast<char>(Serial.read());
    if (ch == '\n') {
      // Accept LF and CRLF, but never fragments of malformed/oversized lines.
      if (used && line[used - 1] == '\r') --used;
      if (!overflow && used == 1 && validStatus(line[0])) {
        if (!bleLinkConnected.load()) applyStatus(line[0], millis());
      } else if (!overflow && used == 1 && line[0] == '?') {
        diagnostic();
      } else if (!overflow && used == 4 && std::memcmp(line, "INFO", 4) == 0) {
        reportIdentity();
      } else if (!overflow && used == 4 && std::memcmp(line, "PAIR", 4) == 0) {
        openPairing();
        reportIdentity();
      }
      used = 0;
      overflow = false;
    } else if (used < sizeof(line)) {
      line[used++] = ch;
    } else {
      overflow = true;
    }
  }
  renderIndication();
  delay(1);
}
