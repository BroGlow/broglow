import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';

// Execute the full embedded page; native layout/keyboard behavior needs a browser.
function popoverPage() {
  const html = readFileSync(new URL('../../assets/popover.html', import.meta.url), 'utf8');
  const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
  const elements = new Map();
  const element = id => {
    if (!elements.has(id)) elements.set(id, {
      id, dataset: {}, attributes: {}, offsetHeight: 200, value: '100', active: false,
      textContent: '', hidden: false, open: false, tagName: 'DIV',
      focus() { context.document.activeElement = this; },
      setAttribute(name, value) { this.attributes[name] = value; },
      matches(selector) {
        return selector === ':active' ? this.active : selector === 'details' && this.tagName === 'DETAILS';
      },
      writes: 0, html: '',
      set innerHTML(value) { this.html = value; this.writes++; },
      get innerHTML() { return this.html; },
    });
    return elements.get(id);
  };
  element('lampbtn').dataset.local = 'lamp';
  const listeners = {}, windowListeners = {}, capture = {}, messages = [];
  let clock = 0, nextTimer = 0;
  const timers = new Map();
  const schedule = (callback, delay, repeat = false) => {
    const id = ++nextTimer;
    timers.set(id, { callback, at: clock + delay, repeat: repeat ? delay : 0 });
    return id;
  };
  const advance = duration => {
    const until = clock + duration;
    for (;;) {
      const next = [...timers].filter(([, timer]) => timer.at <= until).sort((a, b) => a[1].at - b[1].at)[0];
      if (!next) break;
      const [id, timer] = next;
      clock = timer.at;
      if (timer.repeat) timer.at += timer.repeat;
      else timers.delete(id);
      timer.callback();
    }
    clock = until;
  };
  const context = {
    window: { ipc: { postMessage: message => messages.push(JSON.parse(message)) }, devicePixelRatio: 1,
      addEventListener: (name, callback) => { windowListeners[name] = callback; } },
    document: { activeElement: null, hidden: false, getElementById: element,
      addEventListener: (name, callback, options) => { listeners[name] = callback; capture[name] = options; } },
    Date: class extends Date { static now() { return clock; } },
    setTimeout: (callback, delay) => schedule(callback, delay),
    setInterval: (callback, delay) => schedule(callback, delay, true),
    clearTimeout: id => timers.delete(id), clearInterval: id => timers.delete(id),
  };
  runInNewContext(script, context);
  const click = id => listeners.click({ target: { closest: selector => {
    const el = element(id);
    return selector === '[data-local]' && el.dataset.local ? el :
      selector === '[data-cmd]' && el.dataset.cmd ? el : null;
  } } });
  const update = lamp => context.window.__setState({ aggregate: 'none', sessions: [], settings: {}, lamp });
  const commands = name => messages.filter(message => message.cmd === name);
  return { context, element, listeners, windowListeners, capture, messages, click, update, commands, advance };
}

const visibleLabel = element => element.innerHTML.replace(/<[^>]*>/g, '');
const lamp = (changes = {}) => ({
  transport: 'ble', enabled: true, present: false, selected_count: 1, state: 'connected',
  devices: [{ id: 'saved-lamp', state: 'connected', detail: 'Write acknowledged' }],
  ...changes,
});

test('dashboard stays hidden unless explicitly enabled and follows live settings', () => {
  const { context, element } = popoverPage();
  assert.equal(element('dashboardbtn').hidden, true);
  for (const enabled of [true, false, undefined]) {
    context.window.__setState({ aggregate: 'none', sessions: [],
      settings: { showDashboardButton: enabled }, lamp: lamp() });
    assert.equal(element('dashboardbtn').hidden, enabled !== true);
  }
});

test('Settings toggles the saved Dashboard preference, reflects CLI changes and recovers from save failures', () => {
  const { context, element, click, listeners, commands, messages } = popoverPage();
  const refresh = enabled => context.window.__setState({ aggregate: 'none', sessions: [],
    settings: { showDashboardButton: enabled }, lamp: lamp() });
  refresh(false);
  click('settingsbtn');
  const toggle = element('showdashboard');
  assert.equal(toggle.checked, false);
  assert.match(element('groups').innerHTML, /<label[^>]*for="showdashboard"/);
  assert.match(element('groups').innerHTML, /type="checkbox"[^>]*id="showdashboard"/);
  toggle.focus();
  for (const enabled of [true, false]) {
    toggle.checked = enabled;
    listeners.change({ target: toggle });
    assert.deepEqual(commands('set_dashboard_button').at(-1), { cmd: 'set_dashboard_button', enabled });
    assert.equal(element('dashboardbtn').hidden, enabled, 'Await persisted host state');
    refresh(enabled);
    context.window.__configActionResult(null);
    assert.equal(toggle.checked, enabled);
    assert.equal(element('dashboardbtn').hidden, !enabled);
    assert.equal(context.document.activeElement, toggle, 'Keep keyboard focus through refresh');
  }
  refresh(true); // A CLI change uses the same state update.
  assert.equal(toggle.checked, true);
  toggle.checked = false;
  listeners.change({ target: toggle });
  refresh(true); // Failed save leaves the existing preference intact.
  const error = 'Could not save Dashboard setting: <permission denied>';
  context.window.__configActionResult(error);
  assert.equal(toggle.checked, true);
  assert.equal(element('dashboardbtn').hidden, false);
  assert.equal(element('settingserror').hidden, false);
  assert.equal(element('settingserror').textContent, error);
  assert.equal(element('settingserror').innerHTML, '', 'Errors are safe text');
  toggle.checked = false;
  listeners.change({ target: toggle });
  assert.equal(element('settingserror').hidden, true);
  assert.equal(commands('set_dashboard_button').length, 4, 'Retry remains available');
  assert.ok(messages.every(({ cmd }) => ['ready', 'resize', 'set_dashboard_button'].includes(cmd)),
    'Changing appearance does not launch Terminal or control lamps');
});

test('footer uses readable labels and requires fresh worker success, including without USB', () => {
  const { element, update } = popoverPage();
  const button = element('lampbtn');
  for (const transport of ['ble', 'usb']) {
    for (const status of ['searching', 'connecting', 'error', 'unavailable']) {
      // A stale positive per-device state or USB presence cannot turn the dot green.
      update(lamp({ transport, present: true, state: status }));
      assert.equal(visibleLabel(button), 'Lamp');
      assert.ok(!button.innerHTML.includes('dgreen'), `${transport}: ${status}`);
      assert.ok(!/BLE|saved-lamp|Write acknowledged/.test(button.title));
    }
    update(lamp({ transport, present: false }));
    assert.equal(visibleLabel(button), 'Lamp');
    assert.ok(button.innerHTML.includes('dgreen'));
    assert.match(button.title, /connected/i);
    update(lamp({ transport, enabled: false }));
    assert.equal(visibleLabel(button), 'Lamp off');
    assert.ok(!button.innerHTML.includes('dgreen'));
  }
  update(lamp({ devices: [] }));
  assert.ok(!button.innerHTML.includes('dgreen'), 'Missing device success is not a BLE connection');
  assert.ok(!element('groups').innerHTML.includes('lamplist'), 'Main view stays compact');
  assert.equal(button.dataset.cmd, undefined, 'Opening details is not an enable/disable command');
});

test('lamp rows keep IDs and diagnostics collapsed and update without replacing open controls', () => {
  const { element, update, click, listeners, capture, commands } = popoverPage();
  const devices = [
    { id: 'home-id', state: 'connected', detail: 'Write acknowledged' },
    { id: 'work-id', state: 'searching', detail: 'Waiting for advertisement' },
  ];
  const refresh = changes => update(lamp({ selected_count: 2, devices, detail: 'Worker detail', ...changes }));
  refresh();
  assert.equal(visibleLabel(element('lampbtn')), 'Lamps 1/2');
  click('lampbtn');
  const groups = element('groups'), list = element('lamplist');
  const groupBuilds = groups.writes, listBuilds = list.writes;
  assert.match(groups.innerHTML, /id="lampsummary"[^>]*role="status"|role="status"[^>]*id="lampsummary"/);
  assert.match(groups.innerHTML, /<details[^>]*id="lamptechnical"/);
  assert.ok(!/<[^>]*id="lampdetail"[^>]*role="status"/.test(groups.innerHTML));
  assert.equal(element('lamptransport').textContent, 'Bluetooth');
  assert.equal(element('lampstate-0').textContent, 'Connected');
  assert.equal(element('lampstate-1').textContent, 'Looking for lamp');
  assert.equal(element('lampcontrol').textContent, 'Disable lamps');
  assert.equal(element('lampid-0').textContent, 'home-id');
  assert.equal(element('lampid-1').textContent, 'work-id');
  assert.equal(element('lampdetail').textContent, 'Worker detail');
  assert.ok(!/home-id|work-id|Write acknowledged/.test(element('lampsummary').textContent));
  const summaries = [...list.innerHTML.matchAll(/<summary\b[^>]*>([\s\S]*?)<\/summary>/g)];
  assert.equal(summaries.length, 2);
  assert.match(summaries[0][1], /Lamp 1/);
  assert.match(summaries[1][1], /Lamp 2/);
  assert.ok(summaries.every(([, text]) => !/home-id|work-id|Write acknowledged/.test(text)));
  const rows = [...list.innerHTML.matchAll(/<details\b[^>]*>([\s\S]*?)<\/details>/g)];
  rows.forEach(([, html], i) => {
    assert.match(html, new RegExp(`<dd[^>]*id="lampid-${i}"`));
    assert.match(html, new RegExp(`<dd[^>]*id="lampdiagnostic-${i}"`));
  });
  assert.ok(!/<details\b[^>]*\bopen(?:\s|=|>)/.test(list.innerHTML + groups.innerHTML));

  // Native expansion changes geometry only, without rerendering or controlling lamps.
  const details = element('expanded-row');
  details.tagName = 'DETAILS';
  details.open = true;
  assert.ok(capture.toggle === true || capture.toggle?.capture === true);
  const resizeCount = commands('resize').length;
  element('card').offsetHeight = 310;
  listeners.toggle({ target: details });
  assert.equal(commands('resize').length, resizeCount + 1);
  assert.equal(commands('resize').at(-1).height, 310);
  assert.equal(commands('set_led').length, 0);
  assert.equal(list.writes, listBuilds);

  devices[1] = { id: 'work-id', state: 'connected', detail: '<img src=x onerror=bad()> new ACK' };
  refresh();
  assert.equal(visibleLabel(element('lampbtn')), 'Lamps 2/2');
  assert.equal(element('lampstate-1').textContent, 'Connected');
  assert.equal(element('lampdiagnostic-1').textContent, devices[1].detail);
  assert.equal(element('lampdiagnostic-1').innerHTML, '', 'Diagnostic data is written as text');
  assert.equal(list.writes, listBuilds, 'Refresh preserves details nodes and their open state');
  assert.equal(groups.writes, groupBuilds, 'Refresh preserves brightness and control nodes');
  devices[0] = { id: 'home-id', state: 'unavailable', detail: 'Status expired' };
  refresh();
  assert.equal(visibleLabel(element('lampbtn')), 'Lamps 1/2');
  assert.equal(element('lampstate-0').textContent, 'No recent response');
  refresh({ state: 'unavailable' });
  assert.notEqual(element('lampstate-1').textContent, 'Connected');
  refresh({ enabled: false });
  assert.equal(visibleLabel(element('lampbtn')), 'Lamps off');
  assert.equal(element('lampstate-0').textContent, 'Off');
  assert.equal(element('lampstate-1').textContent, 'Off');
  assert.equal(element('lampcontrol').textContent, 'Enable lamps');
  devices[0] = { id: 'home-id', state: 'error', detail: 'Permission denied' };
  devices[1] = { id: '<replacement-id>', state: 'connecting', detail: 'Pairing' };
  refresh();
  assert.ok(list.writes > listBuilds, 'Changed membership replaces device rows');
  assert.equal(groups.writes, groupBuilds, 'Membership changes leave the range control intact');
  assert.equal(element('lampstate-0').textContent, 'Needs attention');
  assert.equal(element('lampstate-1').textContent, 'Connecting');
  assert.equal(element('lampid-1').textContent, '<replacement-id>');
  assert.ok(!list.innerHTML.includes('<replacement-id>'), 'IDs cannot become markup');
});

test('opening details is read-only; lamp controls stay there and Escape returns', () => {
  const { element, listeners, messages, click, update, commands } = popoverPage();
  update(lamp());
  click('lampbtn');
  assert.match(element('groups').innerHTML, />Lamps</);
  assert.equal(commands('set_led').length, 0);
  click('lampcontrol');
  assert.deepEqual(messages.at(-1), { cmd: 'set_led', enabled: false });
  assert.equal(element('lampcontrol').textContent, 'Enable lamp');
  listeners.keydown({ key: 'Escape' });
  assert.ok(!element('groups').innerHTML.includes('lamplist'));
  assert.equal(element('settingsbtn').textContent, 'Settings');
  click('settingsbtn');
  assert.ok(element('groups').innerHTML.includes('Usage tracking'));
  click('settingsbtn');
  assert.ok(!element('groups').innerHTML.includes('Usage tracking'));

  update(lamp({ transport: 'usb', present: true, devices: [] }));
  click('lampbtn');
  assert.equal(element('lamptransport').textContent, 'USB');
  assert.match(element('lamplist').innerHTML, /USB lamp/);
  assert.ok(!element('lamplist').innerHTML.includes('lampforget'), 'USB has no remembered Bluetooth lamp to forget');
  assert.equal(element('brightnesscontrol').hidden, true);
  assert.equal(element('brightnesshint').textContent, 'Brightness control requires Bluetooth.');
  update(lamp({ transport: 'usb', present: false, enabled: false, devices: [] }));
  assert.equal(element('lampcontrol').dataset.cmd, 'get_lamp');
});

test('forget sends exactly the chosen saved ID, including absent and disabled lamps, and awaits the host', () => {
  for (const enabled of [true, false]) {
    const { context, element, click, update, commands } = popoverPage();
    const devices = [
      { id: 'home-id', state: 'connected' },
      { id: 'away-"<>&', state: 'searching' },
    ];
    const refresh = saved => update(lamp({ enabled, selected_count: saved.length, devices: saved }));
    refresh(devices);
    click('lampbtn');
    const list = element('lamplist'), groups = element('groups'), builds = groups.writes;
    assert.equal((list.innerHTML.match(/>Forget lamp<\/button>/g) || []).length, 2);
    assert.match(list.innerHTML, /Stops ClawLight reconnecting\. Pairing stays on the lamp and Mac\./);
    assert.ok(!list.innerHTML.includes(devices[1].id), 'IDs are data, never markup');
    click('lampforget-1');
    assert.deepEqual(commands('forget_lamp'), [{ cmd: 'forget_lamp', id: devices[1].id }]);
    assert.equal(element('lampforget-1').textContent, 'Forgetting…');
    assert.equal(element('lampforget-0').disabled, true);
    assert.equal(element('lampid-0').textContent, 'home-id');
    assert.equal(element('lampid-1').textContent, devices[1].id, 'No optimistic removal before config succeeds');
    click('lampforget-0');
    assert.equal(commands('forget_lamp').length, 1, 'Only one action can be pending');
    refresh(devices);
    assert.equal(element('lampforget-1').disabled, true, 'Heartbeat does not finish the action');

    const slider = element('brightness');
    context.document.activeElement = slider;
    slider.value = '23';
    refresh(devices.slice(0, 1));
    context.window.__configActionResult(null);
    assert.equal((list.innerHTML.match(/>Forget lamp<\/button>/g) || []).length, 1);
    assert.equal(element('lampid-0').textContent, 'home-id', 'The other saved lamp remains');
    assert.equal(element('lampforget-0').disabled, false);
    assert.equal(element('lamperror').hidden, true);
    assert.equal(groups.writes, builds, 'Membership changes preserve the brightness control');
    assert.equal(slider.value, '23');
    assert.equal(commands('set_led').length, 0);
    assert.equal(commands('set_brightness').length, 0);
  }
});

test('forget failure stays visible across refresh, preserves the row and allows retry', () => {
  const { context, element, click, update, commands } = popoverPage();
  update(lamp());
  click('lampbtn');
  click('lampforget-0');
  const error = 'Cannot save <config>: permission denied';
  context.window.__configActionResult(error);
  update(lamp());
  assert.equal(element('lamperror').hidden, false);
  assert.equal(element('lamperror').textContent, error);
  assert.equal(element('lamperror').innerHTML, '', 'Host errors are rendered as text');
  assert.match(element('groups').innerHTML, /id="lamperror"[^>]*role="alert"/);
  assert.equal(element('lampid-0').textContent, 'saved-lamp');
  assert.equal(element('lampforget-0').disabled, false);
  click('lampforget-0');
  assert.equal(element('lamperror').hidden, true);
  assert.deepEqual(commands('forget_lamp').at(-1), { cmd: 'forget_lamp', id: 'saved-lamp' });
  assert.equal(commands('forget_lamp').length, 2);
});

test('brightness saves while absent without enabling lamps and survives focused edits during refresh', () => {
  const { context, element, listeners, messages, click, update, commands } = popoverPage();
  const refresh = percent => update(lamp({
    enabled: false, selected_count: 0, state: 'disabled', devices: [], brightness_percent: percent,
  }));
  refresh(undefined);
  assert.ok(!element('groups').innerHTML.includes('type="range"'));
  click('lampbtn');
  const slider = element('brightness');
  assert.equal(slider.value, '100');
  assert.equal(element('brightnesscontrol').hidden, false);
  assert.match(element('brightnesshint').textContent, /Enable a lamp to preview/);
  const builds = element('groups').writes;
  refresh(75);
  assert.equal(slider.value, '75');
  assert.equal(element('brightnessvalue').textContent, '75%');
  context.document.activeElement = slider;
  slider.value = '30';
  listeners.input({ target: slider });
  refresh(75);
  assert.equal(slider.value, '30');
  assert.equal(context.document.activeElement, slider);
  context.document.activeElement = null;
  slider.active = true;
  slider.value = '20';
  listeners.input({ target: slider });
  refresh(75);
  assert.equal(slider.value, '20');
  assert.equal(element('brightnessvalue').textContent, '20%');
  assert.equal(element('groups').writes, builds);
  assert.equal(commands('set_brightness').length, 0, 'Dragging previews until change is committed');
  listeners.change({ target: slider });
  assert.deepEqual(commands('set_brightness'), [{ cmd: 'set_brightness', percent: 20 }]);
  assert.equal(commands('set_led').length, 0);
  slider.active = false;
  listeners.focusout({ target: slider });
  refresh(20);
  assert.equal(slider.value, '20');
  refresh(55);
  assert.equal(slider.value, '55', 'An idle control accepts external saved changes');
  for (const invalid of ['101', '-1', '1.5', 'invalid']) {
    slider.value = invalid;
    listeners.change({ target: slider });
  }
  assert.equal(commands('set_brightness').length, 1);
  for (const percent of [0, 100]) {
    slider.value = String(percent);
    listeners.change({ target: slider });
    assert.deepEqual(messages.at(-1), { cmd: 'set_brightness', percent });
  }
  assert.equal(element('lampcontrol').dataset.enabled, 'true', 'Saving does not enable the lamp');
});

test('brightness preview throttles live input to the latest value and commits only on change', () => {
  const { element, listeners, click, update, commands, advance } = popoverPage();
  update(lamp());
  click('lampbtn');
  assert.match(element('brightnesshint').textContent, /All three lights.*2 seconds/);
  const slider = element('brightness');
  listeners.pointerdown({ target: slider, pointerId: 1, button: 0 });
  assert.deepEqual(commands('preview_brightness'), [{ cmd: 'preview_brightness', percent: 100 }]);
  for (const value of ['10', '20', '30']) {
    slider.value = value;
    listeners.input({ target: slider });
  }
  assert.equal(element('brightnessvalue').textContent, '30%');
  assert.equal(commands('preview_brightness').length, 1, 'Rapid changes are coalesced');
  advance(110);
  assert.deepEqual(commands('preview_brightness').at(-1), { cmd: 'preview_brightness', percent: 30 });
  assert.equal(commands('preview_brightness').length, 2);
  assert.equal(commands('set_brightness').length, 0);
  slider.value = '42';
  listeners.input({ target: slider });
  listeners.change({ target: slider });
  assert.deepEqual(commands('preview_brightness').at(-1), { cmd: 'preview_brightness', percent: 42 });
  assert.deepEqual(commands('set_brightness'), [{ cmd: 'set_brightness', percent: 42 }]);
  listeners.pointerup({ pointerId: 1 });
  const sent = commands('preview_brightness').length;
  advance(3000);
  assert.equal(commands('preview_brightness').length, sent, 'Release stops renewal and cancels queued old values');
  assert.equal(commands('set_led').length, 0);
});

test('stationary pointer renews preview and keeps its value when focus or host state changes', () => {
  const { context, element, listeners, click, update, commands, advance } = popoverPage();
  update(lamp());
  click('lampbtn');
  const slider = element('brightness'), builds = element('groups').writes;
  listeners.pointerdown({ target: slider, pointerId: 7 });
  slider.value = '25';
  listeners.input({ target: slider });
  advance(110);
  context.document.activeElement = null;
  slider.active = false;
  listeners.focusout({ target: slider });
  update(lamp({ brightness_percent: 100 }));
  assert.equal(slider.value, '25', 'A held pointer protects the unsaved edit even without focus');
  const before = commands('preview_brightness').length;
  advance(1600);
  const renewals = commands('preview_brightness').slice(before);
  assert.ok(renewals.length >= 3 && renewals.length <= 4, 'Holding still renews at a modest rate');
  assert.ok(renewals.every(message => message.percent === 25));
  assert.equal(element('groups').writes, builds);
  listeners.pointercancel({ pointerId: 7 });
  assert.equal(commands('preview_brightness').at(-1).percent, 25);
  const sent = commands('preview_brightness').length;
  advance(3000);
  assert.equal(commands('preview_brightness').length, sent);
  assert.equal(commands('set_brightness').length, 0, 'Preview and cancellation never save');
});

test('popup blur, hiding and leaving the lamp view end preview timers with the final value', () => {
  for (const end of ['blur', 'hidden', 'Escape', 'Back']) {
    const { context, element, listeners, windowListeners, click, update, commands, advance } = popoverPage();
    update(lamp());
    click('lampbtn');
    const slider = element('brightness');
    listeners.pointerdown({ target: slider, pointerId: 1 });
    slider.value = '23';
    listeners.input({ target: slider });
    if (end === 'blur') windowListeners.blur();
    else if (end === 'hidden') { context.document.hidden = true; listeners.visibilitychange(); }
    else if (end === 'Escape') listeners.keydown({ key: 'Escape' });
    else click('settingsbtn');
    assert.equal(commands('preview_brightness').at(-1).percent, 23, end);
    const sent = commands('preview_brightness').length;
    advance(3000);
    assert.equal(commands('preview_brightness').length, sent, end);
    assert.equal(commands('set_brightness').length, 0);
    assert.equal(commands('set_led').length, 0);
  }
});

test('keyboard preview validates percentages and requires an enabled connected wireless lamp', () => {
  const { element, listeners, click, update, commands, advance } = popoverPage();
  update(lamp());
  click('lampbtn');
  const slider = element('brightness');
  for (const invalid of ['101', '-1', '1.5', '', 'invalid']) {
    slider.value = invalid;
    listeners.input({ target: slider });
    listeners.change({ target: slider });
  }
  assert.equal(commands('preview_brightness').length, 0);
  assert.equal(commands('set_brightness').length, 0);
  for (const value of ['0', '100']) {
    slider.value = value;
    listeners.input({ target: slider });
    listeners.change({ target: slider });
    assert.equal(commands('preview_brightness').at(-1).percent, Number(value));
  }
  const sent = commands('preview_brightness').length;
  advance(3000);
  assert.equal(commands('preview_brightness').length, sent, 'Keyboard edits do not start pointer renewal');

  for (const unavailable of [
    { enabled: false }, { state: 'searching' }, { state: 'connecting' }, { state: 'error' },
    { state: 'unavailable' }, { devices: [] },
    { devices: [{ id: 'saved-lamp', state: 'searching' }] }, { transport: 'usb', present: true },
  ]) {
    update(lamp(unavailable));
    slider.value = '45';
    listeners.pointerdown({ target: slider, pointerId: 2 });
    listeners.input({ target: slider });
    listeners.change({ target: slider });
    advance(1100);
    listeners.pointercancel({ pointerId: 2 });
    assert.equal(commands('preview_brightness').length, sent, JSON.stringify(unavailable));
    assert.equal(commands('set_brightness').at(-1).percent, 45, 'Unavailable lamps still accept saved preferences');
  }
  assert.equal(commands('set_led').length, 0);
});
