import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';

test('lamp connection requires a worker success, and BLE needs no USB presence', () => {
  const html = readFileSync(new URL('../../assets/popover.html', import.meta.url), 'utf8');
  const render = html.slice(html.indexOf('function renderLamp()'), html.indexOf('\nfunction relTime('));
  const button = { dataset: {}, setAttribute() {} };
  const detail = {};
  const control = { dataset: {} };
  const slider = { value: '100', matches: () => false };
  const percentage = {};
  const state = { lamp: {} };
  const context = {
    state, view: 'lamp', esc: String,
    document: { getElementById: id => ({
      lampbtn: button, lampdetail: detail, lampcontrol: control,
      brightness: slider, brightnessvalue: percentage,
    })[id] },
  };

  for (const status of ['searching', 'connecting', 'error', 'unavailable']) {
    state.lamp = { transport: 'usb', enabled: true, present: true, state: status, detail: 'Worker detail' };
    runInNewContext(render + '\nrenderLamp();', context);
    assert.ok(!button.innerHTML.includes('dgreen'), status);
    assert.equal(control.dataset.enabled, 'false');
    assert.ok(detail.textContent.includes(status));
    assert.ok(detail.textContent.includes('Worker detail'));
  }

  state.lamp = {
    transport: 'ble', enabled: true, present: false, selected_count: 2, state: 'connected',
    devices: [
      { id: 'home-id', state: 'connected', detail: 'Write acknowledged' },
      { id: 'work-id', state: 'searching', detail: 'Waiting for advertisement' },
    ],
  };
  runInNewContext(render + '\nrenderLamp();', context);
  assert.ok(button.innerHTML.includes('dgreen'));
  assert.ok(button.innerHTML.includes('BLE 1/2'));
  assert.ok(detail.textContent.includes('home-id: connected. Write acknowledged'));
  assert.ok(detail.textContent.includes('work-id: searching. Waiting for advertisement'));

  state.lamp.devices[1] = { id: 'work-id', state: 'connected', detail: 'Write acknowledged' };
  runInNewContext(render + '\nrenderLamp();', context);
  assert.ok(button.innerHTML.includes('BLE 2/2'));

  state.lamp.devices[0] = { id: 'home-id', state: 'unavailable', detail: 'Status expired' };
  runInNewContext(render + '\nrenderLamp();', context);
  assert.ok(button.innerHTML.includes('BLE 1/2'));
  assert.ok(detail.textContent.includes('home-id: unavailable. Status expired'));

  state.lamp.devices = [];
  runInNewContext(render + '\nrenderLamp();', context);
  assert.ok(!button.innerHTML.includes('dgreen'));
  assert.ok(button.innerHTML.includes('BLE 0/2'));

  state.lamp.enabled = false;
  runInNewContext(render + '\nrenderLamp();', context);
  assert.ok(!button.innerHTML.includes('dgreen'));
  assert.equal(control.dataset.enabled, 'true');
  assert.equal(control.dataset.cmd, 'set_led');

  state.lamp.selected_count = 0;
  runInNewContext(render + '\nrenderLamp();', context);
  assert.ok(detail.textContent.includes('clawlight ble add <ID>'));
  assert.ok(detail.textContent.includes('BOOT button 3s'));

  // The main view must not need diagnostic DOM or expose IDs in its tooltip.
  context.view = 'sessions';
  context.document.getElementById = id => id === 'lampbtn' ? button : null;
  runInNewContext(render + '\nrenderLamp();', context);
  assert.equal(button.dataset.cmd, undefined);
  assert.ok(!button.title.includes('home-id'));
  assert.ok(!button.title.includes('Write acknowledged'));
  assert.ok(button.title.includes('View lamp details'));

});

function popoverPage() {
  const html = readFileSync(new URL('../../assets/popover.html', import.meta.url), 'utf8');
  const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
  const elements = new Map();
  const element = id => {
    if (!elements.has(id)) elements.set(id, {
      id, dataset: {}, setAttribute() {}, offsetHeight: 200, value: '100', active: false,
      matches(selector) { return selector === ':active' && this.active; },
      writes: 0, html: '',
      set innerHTML(value) { this.html = value; this.writes++; },
      get innerHTML() { return this.html; },
    });
    return elements.get(id);
  };
  element('lampbtn').dataset.local = 'lamp';
  const listeners = {}, messages = [];
  const context = {
    window: { ipc: { postMessage: message => messages.push(JSON.parse(message)) }, devicePixelRatio: 1 },
    document: { activeElement: null, getElementById: element,
      addEventListener: (name, callback) => { listeners[name] = callback; } },
    setInterval() {},
  };
  runInNewContext(script, context);
  const click = id => listeners.click({ target: { closest: selector => {
    const el = element(id);
    return selector === '[data-local]' && el.dataset.local ? el :
      selector === '[data-cmd]' && el.dataset.cmd ? el : null;
  } } });
  return { context, element, listeners, messages, click };
}

test('opening lamp details is read-only; controls stay there and Escape returns', () => {
  const { context, element, listeners, messages, click } = popoverPage();
  context.window.__setState({ aggregate: 'none', sessions: [], settings: {}, lamp: {
    transport: 'ble', enabled: true, state: 'connected', selected_count: 1,
    devices: [{ id: 'saved-lamp', state: 'connected', detail: 'Write acknowledged' }],
  } });
  assert.ok(!element('groups').innerHTML.includes('lampdetail'));
  click('lampbtn');
  assert.ok(element('groups').innerHTML.includes('Lamp details'));
  assert.ok(element('lampdetail').textContent.includes('saved-lamp'));
  assert.equal(messages.filter(message => message.cmd === 'set_led').length, 0);
  click('lampcontrol');
  assert.deepEqual(messages.at(-1), { cmd: 'set_led', enabled: false });
  assert.equal(element('lampcontrol').textContent, 'Enable lamp');
  listeners.keydown({ key: 'Escape' });
  assert.ok(!element('groups').innerHTML.includes('lampdetail'));
  assert.equal(element('settingsbtn').textContent, 'Settings');
  click('settingsbtn');
  assert.ok(element('groups').innerHTML.includes('Usage tracking'));
  click('settingsbtn');
  assert.ok(!element('groups').innerHTML.includes('Usage tracking'));
});

test('brightness saves while absent without enabling lamps and preserves native edits during refresh', () => {
  const { context, element, listeners, messages, click } = popoverPage();
  const update = percent => context.window.__setState({ aggregate: 'none', sessions: [], settings: {}, lamp: {
    transport: 'ble', enabled: false, present: false, state: 'disabled', selected_count: 0,
    brightness_percent: percent,
  } });
  const changes = () => messages.filter(message => message.cmd === 'set_brightness');
  update(undefined);
  assert.ok(!element('groups').innerHTML.includes('type="range"'));
  click('lampbtn');
  const slider = element('brightness');
  assert.equal(slider.value, '100');
  assert.ok(element('groups').innerHTML.includes('USB brightness is unchanged'));
  const builds = element('groups').writes;
  update(75);
  assert.equal(slider.value, '75');
  assert.equal(element('brightnessvalue').textContent, '75%');

  // Keyboard focus and a mouse drag both survive periodic service refreshes.
  context.document.activeElement = slider;
  slider.value = '30';
  listeners.input({ target: slider });
  update(75);
  assert.equal(slider.value, '30');
  assert.equal(context.document.activeElement, slider);
  context.document.activeElement = null;
  slider.active = true;
  slider.value = '20';
  listeners.input({ target: slider });
  update(75);
  assert.equal(slider.value, '20');
  assert.equal(element('brightnessvalue').textContent, '20%');
  assert.equal(element('groups').writes, builds, 'Do not replace the range DOM during refresh');
  assert.equal(changes().length, 0, 'Dragging only previews until change is committed');
  listeners.change({ target: slider });
  assert.deepEqual(changes(), [{ cmd: 'set_brightness', percent: 20 }]);
  assert.equal(messages.filter(message => message.cmd === 'set_led').length, 0);
  slider.active = false;
  listeners.focusout({ target: slider });
  update(20);
  assert.equal(slider.value, '20');
  update(55);
  assert.equal(slider.value, '55', 'An idle control accepts external saved changes');
  for (const invalid of ['101', '-1', '1.5', 'invalid']) {
    slider.value = invalid;
    listeners.change({ target: slider });
  }
  assert.equal(changes().length, 1);
  for (const percent of [0, 100]) {
    slider.value = String(percent);
    listeners.change({ target: slider });
    assert.deepEqual(changes().at(-1), { cmd: 'set_brightness', percent });
  }
  assert.equal(element('lampcontrol').dataset.enabled, 'true', 'Saving does not enable the lamp');
});
