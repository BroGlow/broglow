import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import test from 'node:test';
import { runInNewContext } from 'node:vm';

const { render } = createRequire(import.meta.url)('../../assets/popover/document.js');
const popoverAssets = {
  template: readFileSync(new URL('../../assets/popover.html', import.meta.url), 'utf8'),
  styles: readFileSync(new URL('../../assets/popover/popover.css', import.meta.url), 'utf8'),
  script: readFileSync(new URL('../../assets/popover/popover.js', import.meta.url), 'utf8'),
  presentation: readFileSync(new URL('../../assets/popover/presentation.js', import.meta.url), 'utf8'),
  icons: Object.fromEntries(['red', 'yellow', 'green', 'none'].map(color => [color,
    'data:image/png;base64,' + readFileSync(new URL(`../../assets/icons/broglow-${color}.png`, import.meta.url)).toString('base64')])),
};

// Execute the full embedded page; native layout/keyboard behavior needs a browser.
function popoverPage() {
  const html = render(popoverAssets);
  const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(match => match[1]);
  const elements = new Map();
  const element = id => {
    if (!elements.has(id)) elements.set(id, {
      id, dataset: {}, attributes: {}, offsetHeight: 200, value: '100', active: false,
      textContent: '', hidden: false, open: false, tagName: 'DIV',
      focus() { context.document.activeElement = this; },
      blur() { if (context.document.activeElement === this) context.document.activeElement = null; },
      click() { click(id); },
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
  for (const script of scripts) runInNewContext(script, context);
  const click = id => listeners.click({ target: { closest: selector => {
    const el = element(id);
    return selector === '[data-local]' && el.dataset.local ? el :
      selector === '[data-cmd]' && el.dataset.cmd ? el : null;
  } } });
  const pushState = (changes, resetNavigation = false) => context.window.__setState(
    { aggregate: 'none', sessions: [], settings: {}, lamp: lamp(), ...changes }, resetNavigation);
  const update = lamp => pushState({ lamp });
  const commands = name => messages.filter(message => message.cmd === name);
  return { context, element, listeners, windowListeners, capture, messages, click, pushState, update, commands, advance };
}

const visibleLabel = element => element.innerHTML.replace(/<[^>]*>/g, '');
const lamp = (changes = {}) => ({
  transport: 'ble', enabled: true, present: false, selected_count: 1, state: 'connected',
  devices: [{ id: 'saved-lamp', state: 'connected', detail: 'Write acknowledged' }],
  ...changes,
});

function lampPage(changes = {}) {
  const page = popoverPage();
  page.update(lamp(changes));
  page.click('lampbtn');
  return page;
}

const session = (id, changes = {}) => ({
  id, name: 'Task ' + id, project: 'broglow', status: 'active',
  lastUpdated: '2026-01-01T12:00:00Z', ...changes,
});

function sessionMeta(element, id) {
  const html = element('groups').innerHTML;
  return html.slice(html.indexOf(`data-id="${id}"`)).match(/<span class="smeta">(.*?)<\/span><\/div>/)[1];
}

test('a state push cannot replace Open Codex before its pointer click is delivered', () => {
  const { context, element, listeners, windowListeners, click, commands, advance } = popoverPage();
  const refresh = name => context.window.__setState({ aggregate: 'red', settings: {}, lamp: lamp(),
    sessions: [{ id: 'codex-task', name, project: 'broglow', status: 'needs_help', capacityBlocked: true, lastUpdated: new Date().toISOString() }] });
  refresh('Original title');
  const focus = element('focus-codex-task');
  focus.dataset = { cmd: 'focus', id: 'codex-task' };
  focus.closest = selector => selector === '[data-cmd]' ? focus : null;
  const down = () => listeners.pointerdown({ target: focus, button: 0, pointerId: 7 });
  down();
  const writes = element('groups').writes;
  refresh('Updated while pressing Focus');
  assert.equal(element('groups').writes, writes, 'Keep the actual pointer target attached until click');
  listeners.pointerup({ pointerId: 7 });
  assert.equal(element('groups').writes, writes, 'Pointerup precedes click dispatch');
  click(focus.id);
  assert.deepEqual(commands('focus'), [{ cmd: 'focus', id: 'codex-task' }]);
  advance(0);
  assert.match(element('groups').innerHTML, /Updated while pressing Focus/);
  down(); refresh('After cancellation');
  listeners.pointercancel({ pointerId: 7 }); advance(0);
  assert.match(element('groups').innerHTML, /After cancellation/);
  down(); refresh('After blur'); windowListeners.blur();
  assert.match(element('groups').innerHTML, /After blur/, 'A hidden popover must not retain a stale pointer');
});

test('capacity rows explain recovery while keeping session metadata, focus and dismissal', () => {
  const { element, pushState, click, commands } = popoverPage();
  const capacity = session('capacity', {
    name: 'Ship <fix> "now"', project: 'bro<glow>', projectPath: '/work/bro<glow>',
    status: 'needs_help', capacityBlocked: true,
    agent: { label: 'Codex', icon: 'M8 1v14', badge: 'cx' },
    host: { kind: 'app', name: 'Codex <app>' },
  });
  for (const groupByProject of [false, true]) {
    pushState({ aggregate: 'red', sessions: [capacity, session('question', { status: 'needs_help' })],
      settings: { groupByProject } });
    const html = element('groups').innerHTML;
    assert.match(html, /class="srow helprow"[^>]*data-id="capacity"[^>]*title="Open Codex"/);
    assert.match(html, /Ship &lt;fix&gt; &quot;now&quot;/);
    assert.match(html, /bro&lt;glow&gt;/);
    assert.match(html, /aria-label="Codex &lt;app&gt;"/);
    assert.match(html, /class="sreason">Model at capacity<\/span>/);
    assert.match(html, /id="capacity-capacity">Choose another model or retry in Codex\.<\/span>/);
    assert.match(html, /class="focusbtn" type="button"[^>]*aria-label="Open Codex: Ship &lt;fix&gt; &quot;now&quot;"[^>]*aria-describedby="capacity-capacity">Open Codex<\/button>/);
    assert.equal((html.match(/>Open Codex<\/button>/g) || []).length, 1);
    assert.equal((html.match(/>Focus<\/button>/g) || []).length, 1, 'Ordinary needs-help stays unchanged');
  }
  const focus = element('focus-capacity');
  focus.dataset = { cmd: 'focus', id: 'capacity' };
  click(focus.id);
  assert.deepEqual(commands('focus'), [{ cmd: 'focus', id: 'capacity' }]);
  const dismiss = element('dismiss-capacity');
  dismiss.dataset = { cmd: 'clear_session', id: 'capacity' };
  click(dismiss.id);
  assert.deepEqual(commands('clear_session'), [{ cmd: 'clear_session', id: 'capacity' }]);
  assert.equal(commands('focus').length, 1, 'Dismiss does not also open Codex');
  pushState({ sessions: [{ ...capacity, status: 'active' }] });
  assert.doesNotMatch(element('groups').innerHTML, /Model at capacity|Choose another model|>Open Codex<\/button>/,
    'A new attempt removes the warning even if an older producer leaves the flag set');
});

test('only the host reset flag returns Lamps and Settings to current chats', () => {
  for (const [button, view] of [['lampbtn', 'lamp'], ['settingsbtn', 'settings']]) {
    const { element, click, pushState, windowListeners, advance, commands } = popoverPage();
    pushState({ sessions: [session('old')] });
    click(button);
    windowListeners.blur();
    advance(60 * 60 * 1000);
    pushState({ sessions: [session('while-closed')] });
    assert.equal(element('groups').dataset.view, view, 'JS never decides the close timeout');
    pushState({ sessions: [session('current')] }, true);
    assert.equal(element('groups').dataset.view, 'sessions');
    assert.equal(element('settingsbtn').textContent, 'Settings');
    assert.match(element('groups').innerHTML, /Task current/);
    assert.doesNotMatch(element('groups').innerHTML, /Task old|Task while-closed/);
    click(button);
    pushState({ sessions: [session('later')] });
    assert.equal(element('groups').dataset.view, view, 'Reset applies only to that host push');
    assert.equal(commands('set_led').length, 0);
    assert.equal(commands('set_brightness').length, 0);
  }
});

test('a host navigation reset releases a stale session pointer before rendering fresh chats', () => {
  const { element, listeners, pushState } = popoverPage();
  pushState({ sessions: [session('old')] });
  const row = element('focus-old');
  row.dataset = { cmd: 'focus', id: 'old' };
  row.closest = () => row;
  listeners.pointerdown({ target: row, button: 0, pointerId: 1 });
  pushState({ sessions: [session('held')] });
  assert.match(element('groups').innerHTML, /Task old/);
  pushState({ sessions: [session('current')] }, true);
  assert.match(element('groups').innerHTML, /Task current/);
  pushState({ sessions: [session('later')] });
  assert.match(element('groups').innerHTML, /Task later/);
});

test('sessions dismiss without focusing, show failures, and return on fresh host activity', () => {
  const { context, element, click, commands } = popoverPage();
  const sessions = ['active', 'inactive', 'needs_help'].map((status, i) => ({
    id: 'session-' + i, name: 'Task <one> "quoted"', project: 'project', status,
    lastUpdated: new Date().toISOString(),
    agent: { label: 'Codex', icon: 'M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5z', badge: 'cx' },
  }));
  const refresh = sessions => context.window.__setState({ aggregate: 'red', sessions, settings: {}, lamp: lamp() });
  refresh(sessions);
  const html = element('groups').innerHTML;
  assert.equal((html.match(/class="dismissbtn"/g) || []).length, 3);
  assert.match(html, /type="button"[^>]*data-cmd="clear_session"/);
  assert.match(html, /aria-label="Dismiss Task &lt;one&gt; &quot;quoted&quot; from BroGlow"/);
  const dismiss = element('dismiss-session-1');
  dismiss.dataset = { cmd: 'clear_session', id: 'session-1' };
  dismiss.focus();
  refresh(sessions);
  assert.equal(context.document.activeElement, dismiss);
  click(dismiss.id);
  assert.deepEqual(commands('clear_session'), [{ cmd: 'clear_session', id: 'session-1' }]);
  assert.equal(commands('focus').length, 0);
  assert.equal(commands('hide').length, 0);
  assert.match(element('groups').innerHTML, /dismiss-session-1/, 'Wait for the saved host state');
  context.window.__configActionResult('Could not dismiss session: <locked>');
  assert.equal(element('sessionerror').textContent, 'Could not dismiss session: <locked>');
  assert.equal(element('sessionerror').innerHTML, '');
  assert.equal(element('sessionerror').hidden, false);
  click(dismiss.id);
  assert.equal(element('sessionerror').hidden, true);
  refresh(sessions.filter(s => s.id !== 'session-1'));
  context.window.__configActionResult(null);
  assert.doesNotMatch(element('groups').innerHTML, /dismiss-session-1/);
  assert.equal(element('aggline').textContent, '2 live sessions · 1 needs help');
  refresh(sessions.map(s => ({ ...s, status: 'active' })));
  assert.match(element('groups').innerHTML, /dismiss-session-1/);
  refresh([]);
  assert.equal(element('aggline').textContent, 'No live sessions');
});

test('Clear paused appears only with yellow sessions and waits for host state, including failures', () => {
  const { context, element, click, commands } = popoverPage();
  const sessions = ['active', 'inactive', 'needs_help', 'inactive'].map((status, i) => ({
    id: 'session-' + i, name: 'Task ' + i, project: 'project', status,
    lastUpdated: new Date().toISOString(),
  }));
  const refresh = sessions => context.window.__setState({ aggregate: 'red', sessions, settings: {}, lamp: lamp() });
  refresh(sessions);
  assert.equal((element('groups').innerHTML.match(/>Clear paused<\/button>/g) || []).length, 1);
  assert.match(element('groups').innerHTML, /Paused<\/div><button[^>]*type="button"[^>]*data-cmd="clear_paused"/);
  const button = element('clearpaused');
  button.dataset.cmd = 'clear_paused';
  button.focus();
  refresh(sessions);
  assert.equal(context.document.activeElement, button);
  click(button.id);
  assert.deepEqual(commands('clear_paused'), [{ cmd: 'clear_paused' }]);
  assert.equal(commands('clear_session').length, 0, 'The host checks current statuses in one action');
  assert.equal(commands('focus').length, 0);
  assert.equal(commands('hide').length, 0);
  assert.match(element('groups').innerHTML, /dismiss-session-1/, 'No optimistic removal');
  context.window.__configActionResult('Could not dismiss sessions: <locked>');
  assert.equal(element('sessionerror').textContent, 'Could not dismiss sessions: <locked>');
  assert.equal(element('sessionerror').innerHTML, '');
  assert.equal(element('sessionerror').hidden, false);
  click(button.id);
  assert.equal(element('sessionerror').hidden, true);
  refresh(sessions.filter(s => s.status !== 'inactive'));
  context.window.__configActionResult(null);
  const remaining = element('groups').innerHTML;
  assert.match(remaining, /dismiss-session-0/);
  assert.match(remaining, /dismiss-session-2/);
  assert.doesNotMatch(remaining, /dismiss-session-[13]|Clear paused/);
  refresh([]);
  assert.doesNotMatch(element('groups').innerHTML, /Clear paused/);
  refresh(sessions);
  click('settingsbtn');
  assert.doesNotMatch(element('groups').innerHTML, /Clear paused/);
});

test('session meta shows the agent icon and name, or the icon alone when names are off', () => {
  const { context, element } = popoverPage();
  const sessions = [
    session('claude', { agent: { label: 'Claude', icon: 'M8 1.5v13', badge: '' } }),
    session('future', { agent: { label: 'Mystery <b>', badge: 'my' } }),
  ];
  const refresh = settings => context.window.__setState({ aggregate: 'green', sessions, settings, lamp: lamp() });
  const meta = id => sessionMeta(element, id);

  refresh({}); // Names show by default, including for Claude.
  assert.match(meta('claude'), /<svg[^>]*aria-hidden="true"/);
  assert.equal(visibleLabel({ innerHTML: meta('claude') }), 'Claude · broglow');
  assert.match(meta('future'), /aria-hidden="true">my<\/span>/);
  assert.equal(visibleLabel({ innerHTML: meta('future') }), 'myMystery &lt;b&gt; · broglow');

  refresh({ showAgentNames: false });
  assert.match(meta('claude'), /role="img" aria-label="Claude"/);
  assert.equal(visibleLabel({ innerHTML: meta('claude') }), 'broglow', 'No separator without a name');
  assert.match(meta('claude'), /title="Claude"/, 'Hover still names the agent');
  assert.equal(visibleLabel({ innerHTML: meta('future') }), 'mybroglow');
});

test('a host glyph follows the agent and names the host on hover', () => {
  const { context, element } = popoverPage();
  const agent = { label: 'Claude', icon: 'M8 1.5v13', badge: 'cc' };
  const sessions = [
    session('app', { agent, host: { kind: 'app', name: 'Claude app' } }),
    session('tty', { agent, host: { kind: 'terminal', name: 'Ghost<ty>' } }),
    session('odd', { agent, host: { kind: 'future', name: 'Somewhere' } }),
  ];
  const refresh = settings => context.window.__setState({ aggregate: 'green', sessions, settings, lamp: lamp() });
  const meta = id => sessionMeta(element, id);
  const text = html => html.replace(/<svg[\s\S]*?<\/svg>/g, '[svg]').replace(/<[^>]*>/g, '');

  refresh({});
  assert.match(meta('app'), /role="img" aria-label="Claude app"><title>Claude app<\/title>/);
  assert.equal(text(meta('app')), '[svg]Claude [svg] · broglow');
  assert.match(meta('tty'), /aria-label="Ghost&lt;ty&gt;"/, 'Host names are escaped');
  assert.equal(text(meta('odd')), '[svg]Claude · broglow', 'Unknown kinds draw no glyph');

  refresh({ showAgentNames: false });
  assert.match(meta('app'), /role="img" aria-label="Claude app"/);
  assert.equal(text(meta('app')), '[svg][svg]broglow');
});

test('Settings uses native radios for mixed sessions and a checkbox for usage tracking', () => {
  const { context, element, click, listeners, commands } = popoverPage();
  const refresh = settings => context.window.__setState({ aggregate: 'none', sessions: [],
    settings, lamp: lamp() });
  refresh({});
  click('settingsbtn');
  const html = element('groups').innerHTML;
  assert.match(html, /role="radiogroup" aria-labelledby="mixedlabel"/);
  for (const value of ['any_inactive', 'active_wins']) {
    assert.match(html, new RegExp(`<label[^>]*for="yellow-${value}"><input type="radio" name="yellowmode"`));
  }
  assert.match(html, /<label[^>]*for="trackusage"><input type="checkbox" id="trackusage"/);
  assert.doesNotMatch(html, /dollar|API rates|\$/i);
  assert.equal(element('yellow-any_inactive').checked, true, 'Default mode is selected');
  assert.equal(element('trackusage').checked, false, 'Usage tracking is off by default');
  assert.equal(element('showusageinmenubar').checked, true, 'Menu-bar usage defaults on');
  assert.equal(element('showusageinmenubar').disabled, true, 'Menu-bar usage requires tracking');
  assert.match(html, /id="usagehint">[^<]*<\/span><\/span><\/label><label[^>]*for="showusageinmenubar"><input type="checkbox" id="showusageinmenubar" aria-describedby="showusageinmenubarhint"/,
    'The menu-bar checkbox is directly below Track usage');

  const radio = element('yellow-active_wins');
  Object.assign(radio, { name: 'yellowmode', value: 'active_wins', checked: true });
  radio.focus();
  listeners.change({ target: radio });
  assert.deepEqual(commands('set_yellow_mode').at(-1), { cmd: 'set_yellow_mode', mode: 'active_wins' });
  refresh({ yellowMode: 'active_wins' });
  assert.equal(radio.checked, true);
  assert.equal(context.document.activeElement, radio, 'Arrow-key users keep focus through refresh');

  const usage = element('trackusage');
  for (const enabled of [true, false]) {
    usage.checked = enabled;
    listeners.change({ target: usage });
    assert.deepEqual(commands('set_usage').at(-1), { cmd: 'set_usage', enabled });
    refresh({ usageEnabled: enabled });
    assert.equal(usage.checked, enabled);
  }
});

for (const { id, key, command, defaultValue } of [
  { id: 'showagentnames', key: 'showAgentNames', command: 'set_agent_names', defaultValue: true },
  { id: 'groupbyproject', key: 'groupByProject', command: 'set_group_by_project', defaultValue: false },
  { id: 'autoaddforgottenusb', key: 'autoAddForgottenUsb', command: 'set_auto_add_forgotten_usb', defaultValue: true },
  { id: 'popoverreset', key: 'popoverResetEnabled', command: 'set_popover_reset', defaultValue: true },
  { id: 'showusageinmenubar', key: 'showUsageInMenuBar', command: 'set_usage_in_menu_bar', defaultValue: true },
]) {
  test(`Settings saves ${key} while preserving the pending choice and keyboard focus`, () => {
    const { context, element, click, listeners, commands, pushState } = popoverPage();
    const refresh = settings => pushState({ settings: { usageEnabled: true, ...settings } });
    refresh({});
    click('settingsbtn');
    const toggle = element(id);
    assert.equal(toggle.checked, defaultValue);
    toggle.focus();
    const builds = element('groups').writes;

    for (const enabled of [!defaultValue, defaultValue]) {
      toggle.checked = enabled;
      listeners.change({ target: toggle });
      assert.deepEqual(commands(command).at(-1), { cmd: command, enabled });
      refresh({ [key]: !enabled });
      assert.equal(toggle.checked, enabled, 'A racing state push cannot undo the pending edit');
      context.window.__configActionResult(null);
      refresh({ [key]: enabled });
      assert.equal(toggle.checked, enabled);
      assert.equal(context.document.activeElement, toggle);
    }
    toggle.checked = !defaultValue;
    listeners.change({ target: toggle });
    refresh({ [key]: defaultValue });
    context.window.__configActionResult('Could not save setting');
    assert.equal(toggle.checked, defaultValue, 'A failed write restores the saved value');
    assert.equal(element('settingserror').textContent, 'Could not save setting');
    assert.equal(element('groups').writes, builds, 'Keep the control attached during host refreshes');
  });
}

test('Track usage immediately enables its menu-bar checkbox and retains the saved display choice', () => {
  const { context, element, click, listeners, commands, pushState } = popoverPage();
  pushState({ settings: { usageEnabled: true, showUsageInMenuBar: false } });
  click('settingsbtn');
  const tracking = element('trackusage'), menuBar = element('showusageinmenubar');
  assert.equal(menuBar.checked, false);
  assert.equal(menuBar.disabled, false);
  tracking.checked = false;
  listeners.change({ target: tracking });
  assert.equal(menuBar.disabled, true, 'Disable the dependent checkbox before saving');
  assert.equal(menuBar.checked, false, 'Turning tracking off preserves its display preference');
  pushState({ settings: { usageEnabled: true, showUsageInMenuBar: false } });
  assert.equal(menuBar.disabled, true, 'A racing snapshot preserves pending tracking opt-out');
  context.window.__configActionResult('Could not save setting');
  assert.equal(tracking.checked, true);
  assert.equal(menuBar.disabled, false, 'Rejected opt-out restores the saved dependency');
  pushState({ settings: { usageEnabled: false, showUsageInMenuBar: false } });
  assert.equal(menuBar.disabled, true);
  menuBar.checked = true;
  listeners.change({ target: menuBar });
  assert.equal(commands('set_usage_in_menu_bar').length, 0, 'Disabled display control cannot save');
  tracking.checked = true;
  listeners.change({ target: tracking });
  assert.equal(menuBar.disabled, false, 'Tracking opt-in enables the dependent checkbox immediately');
  assert.equal(menuBar.checked, false, 'Opting back in preserves the previous display choice');
  assert.equal(commands('set_usage_in_menu_bar').length, 0, 'Tracking never rewrites the display preference');
});

test('popover return time defaults to five minutes and follows the enabled checkbox immediately', () => {
  const { context, element, click, listeners, pushState } = popoverPage();
  click('settingsbtn');
  const html = element('groups').innerHTML;
  assert.match(html, /role="group" aria-label="Return to chats after closing"/);
  assert.match(html, /While the popover is closed\./);
  assert.match(html, /Return to chats after<\/span><\/label><input type="number"[^>]*min="1" max="1440" step="1" aria-label="Minutes closed before returning to chats"/,
    'The inline number has its own accessible name and is outside the checkbox label');
  const toggle = element('popoverreset'), minutes = element('popoverresetminutes');
  assert.equal(toggle.checked, true);
  assert.equal(minutes.value, '5');
  assert.equal(minutes.disabled, false);
  toggle.checked = false;
  listeners.change({ target: toggle });
  assert.equal(minutes.disabled, true);
  pushState({ settings: { popoverResetEnabled: true, popoverResetMinutes: 15 } });
  assert.equal(minutes.disabled, true, 'Racing saved state preserves the pending opt-out');
  context.window.__configActionResult('Could not save setting');
  assert.equal(minutes.disabled, false, 'Rejected opt-out restores the saved enabled state');
  assert.equal(minutes.value, '15');
  pushState({ settings: { popoverResetEnabled: false, popoverResetMinutes: 15 } });
  assert.equal(toggle.checked, false);
  assert.equal(minutes.disabled, true);
  toggle.checked = true;
  listeners.change({ target: toggle });
  assert.equal(minutes.disabled, false);
  assert.equal(minutes.value, '15', 'Opting out retains the configured duration');
});

test('popover minutes preserve focused drafts and pending saves, and restore rejected values', () => {
  const { context, element, click, listeners, pushState, commands } = popoverPage();
  click('settingsbtn');
  const input = element('popoverresetminutes');
  const builds = element('groups').writes;
  input.focus();
  input.value = '';
  pushState({ settings: { popoverResetMinutes: 5 } });
  assert.equal(input.value, '', 'A refresh cannot replace a half-typed draft');
  assert.equal(commands('set_popover_reset_minutes').length, 0);
  input.value = '15';
  listeners.change({ target: input });
  assert.deepEqual(commands('set_popover_reset_minutes').at(-1), { cmd: 'set_popover_reset_minutes', minutes: 15 });
  pushState({ settings: { popoverResetMinutes: 5 } });
  assert.equal(input.value, '15');
  assert.equal(context.document.activeElement, input);
  input.blur();
  listeners.focusout({ target: input });
  assert.equal(input.value, '15', 'Pending value survives blur before the write completes');
  pushState({ settings: { popoverResetMinutes: 15 } });
  context.window.__configActionResult(null);
  assert.equal(input.value, '15');
  input.focus();
  input.value = '20';
  listeners.change({ target: input });
  pushState({ settings: { popoverResetMinutes: 15 } });
  context.window.__configActionResult('Could not save return time: <locked>');
  assert.equal(input.value, '15', 'A rejected save restores the saved value even while focused');
  assert.equal(context.document.activeElement, input);
  assert.equal(element('settingserror').textContent, 'Could not save return time: <locked>');
  assert.equal(element('settingserror').innerHTML, '');
  assert.equal(element('settingserror').hidden, false);
  assert.equal(element('groups').writes, builds);
});

test('popover minutes accept only whole minutes from one through 1440 and cannot save while disabled', () => {
  const { context, element, click, listeners, pushState, commands } = popoverPage();
  click('settingsbtn');
  const input = element('popoverresetminutes');
  input.focus();
  for (const value of ['', '0', '-1', '1441', '1.5', 'invalid']) {
    input.value = value;
    listeners.change({ target: input });
    assert.equal(commands('set_popover_reset_minutes').length, 0);
    assert.equal(element('settingserror').textContent, 'Use a whole number of minutes from 1 to 1440.');
  }
  for (const minutes of [1, 1440, 10]) {
    input.value = String(minutes);
    listeners.change({ target: input });
    assert.deepEqual(commands('set_popover_reset_minutes').at(-1), { cmd: 'set_popover_reset_minutes', minutes });
    assert.equal(element('settingserror').hidden, true, 'A valid retry clears the error');
    pushState({ settings: { popoverResetMinutes: minutes } });
    context.window.__configActionResult(null);
  }
  pushState({ settings: { popoverResetEnabled: false, popoverResetMinutes: 10 } });
  input.value = '25';
  listeners.change({ target: input });
  assert.equal(commands('set_popover_reset_minutes').length, 3);
});

test('USB re-add preference is shown only on platforms with Bluetooth setup', () => {
  const { click, element, pushState } = popoverPage();
  click('settingsbtn');
  for (const supported of [true, false]) {
    pushState({ lamp: lamp({ bluetooth_supported: supported }) });
    assert.equal(element('usblampsettings').hidden, !supported);
  }
});

test('grouping by project lists sessions under directory headings with the status in each row', () => {
  const { context, element } = popoverPage();
  const now = Date.parse('2026-01-01T12:00:00Z');
  const at = minutes => new Date(now - minutes * 60000).toISOString();
  const agent = { label: 'Claude', icon: 'M8 1.5v13', badge: 'cc' };
  const session = (id, status, projectPath, project, minutes) =>
    ({ id, name: 'Task ' + id, status, projectPath, project, lastUpdated: at(minutes), agent });
  const sessions = [
    session('focus', 'inactive', '/d/broglow', 'broglow', 17),
    session('notes', 'inactive', '', 'No project', 120),
    session('review', 'active', '/a/recco', 'a/recco', 1),
    session('grouping', 'active', '/d/broglow', 'broglow', 0),
    session('deploy', 'inactive', '/b/recco', 'b/recco', 42),
    session('help', 'needs_help', '/a/recco', 'a/recco', 3),
    session('odd', 'future', '/a/recco', 'a/recco', 0),
  ];
  const refresh = settings => context.window.__setState({ aggregate: 'red', sessions, settings, lamp: lamp() });
  const text = html => html.replace(/<svg[\s\S]*?<\/svg>/g, '').replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();

  refresh({ groupByProject: true });
  const html = element('groups').innerHTML;
  const order = [...html.matchAll(/class="projhead"[^>]*>(?:<svg[\s\S]*?<\/svg>)?<span>([^<]*)<\/span>|data-id="([^"]+)" title="Open session"/g)]
    .map(m => m[1] ?? m[2]);
  assert.deepEqual(order, ['a/recco', 'help', 'review', 'broglow', 'grouping', 'focus', 'b/recco', 'deploy', 'No project', 'notes'],
    'Most urgent project first, then recency; status then recency inside; unknown directory last; unknown statuses dropped');
  assert.match(html, /data-cmd="clear_paused"/, 'Grouped sessions still offer Clear paused');
  assert.doesNotMatch(html, /Needs help<\/div>|Working<\/div>|Paused<\/div>/, 'No status headings');
  assert.match(html, /<div class="projhead" role="heading" aria-level="3" title="\/a\/recco" aria-label="a\/recco, \/a\/recco"><svg/,
    'The path is hover text and part of the accessible name');
  assert.match(html, /<div class="projhead" role="heading" aria-level="3"><span>No project<\/span>/, 'No glyph or path for an unknown directory');
  const row = id => html.slice(html.indexOf(`data-id="${id}" title`)).match(/<span class="smeta">(.*?)<\/span><\/div>/)[1];
  assert.equal(text(row('help')), 'Claude · needs help');
  assert.equal(text(row('grouping')), 'Claude · working');
  assert.equal(text(row('deploy')), 'Claude · paused');
  assert.match(element('chips').innerHTML, /title="1 needs help" aria-label="1 needs help"/);
  assert.match(element('chips').innerHTML, /title="3 paused" aria-label="3 paused"/);

  refresh({});
  const flat = element('groups').innerHTML;
  assert.match(flat, /Needs help<\/div>/);
  assert.doesNotMatch(flat, /projhead|By project/);
  assert.equal(text(flat.slice(flat.indexOf('data-id="deploy" title')).match(/<span class="smeta">(.*?)<\/span><\/div>/)[1]), 'Claude · b/recco');
  assert.equal(text(flat.slice(flat.indexOf('data-id="notes" title')).match(/<span class="smeta">(.*?)<\/span><\/div>/)[1]), 'Claude · No project');

  context.window.__setState({ aggregate: 'red', sessions: [session('odd', 'future', '/x', 'x', 0)], settings: { groupByProject: true }, lamp: lamp() });
  assert.doesNotMatch(element('groups').innerHTML, /By project/, 'No heading without any row to show');

  // Equal urgency and time: the path decides, and nothing paused means no Clear paused.
  const tied = ['/z', '/m', '/a'].map(path => ({ ...session('t' + path.slice(1), 'active', path, path.slice(1), 0), lastUpdated: at(0) }));
  context.window.__setState({ aggregate: 'green', sessions: tied, settings: { groupByProject: true }, lamp: lamp() });
  const tiedHtml = element('groups').innerHTML;
  assert.deepEqual([...tiedHtml.matchAll(/class="projhead"[^>]*><svg[\s\S]*?<\/svg><span>([^<]*)<\/span>/g)].map(m => m[1]), ['a', 'm', 'z']);
  assert.doesNotMatch(tiedHtml, /clearpaused/);
});

test('usage keeps Claude and Codex quotas separate from opencode API tokens and the all-agent total', () => {
  const { element, pushState } = popoverPage();
  const icons = [
    'M8 1.5v13M1.5 8h13M3.4 3.4l9.2 9.2M12.6 3.4l-9.2 9.2',
    'M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5z',
    'M5.5 2.5H3v11h2.5M10.5 2.5H13v11h-2.5',
  ];
  pushState({ settings: { usageEnabled: true, showAgentNames: false }, usage: { todayTokens: 1312400, agents: [
    { harness: 'claude', label: 'Claude', badge: 'cc', icon: icons[0], todayTokens: 1240000,
      limits: [{ label: '5h block', usedPercent: 62, reset: '4:00 PM' },
        { label: 'Week', usedPercent: 31, reset: null }] },
    { harness: 'codex', label: 'Codex', badge: 'cx', icon: icons[1], todayTokens: 60000,
      limits: [{ label: '5h block', usedPercent: 12, reset: '5:00 PM' }] },
    { harness: 'opencode', label: 'opencode', badge: 'oc', icon: icons[2], todayTokens: 12400, limits: [] },
  ] } });
  const html = element('usage').innerHTML;
  assert.equal((html.match(/role="heading" aria-level="2">Usage<\/div>/g) || []).length, 1);
  assert.equal((html.match(/role="heading" aria-level="3"/g) || []).length, 3);
  assert.doesNotMatch(html, /Usage ·|class="badge"/);
  const sections = [...html.matchAll(/<section[^>]*>(.*?)<\/section>/g)].map(m => m[1]);
  assert.equal(sections.length, 3);
  for (const [i, label] of ['Claude', 'Codex', 'opencode'].entries()) {
    assert.ok(sections[i].startsWith('<div class="uagenthead" role="heading" aria-level="3">' +
      `<svg class="agicon" viewBox="0 0 16 16" aria-hidden="true"><path d="${icons[i]}"/></svg>${label}</div>`),
      'Usage keeps named agent headings when session names are hidden');
  }
  assert.match(sections[0], /Today ≈ 1\.2M tokens/);
  assert.match(sections[0], /62%.*4:00 PM/);
  assert.match(sections[0], /31%/);
  assert.doesNotMatch(sections[0], /12%/);
  assert.match(sections[1], /Today ≈ 60k tokens/);
  assert.match(sections[1], /12%.*5:00 PM/);
  assert.doesNotMatch(sections[1], /62%|31%/);
  assert.match(sections[2], /Today ≈ 12k tokens/);
  assert.doesNotMatch(sections[2], /%|progressbar/);
  assert.match(visibleLabel(element('usage')), /All agents today ≈ 1\.3M tokens/);
  assert.doesNotMatch(visibleLabel(element('usage')), /\$|74%|105%/);
  assert.equal(element('usage').hidden, false);
});

test('usage escapes agent identity fields and keeps the unknown-agent badge fallback', () => {
  const { element, pushState } = popoverPage();
  pushState({ settings: { usageEnabled: true }, usage: { todayTokens: 2, agents: [
    { label: 'Known <agent>', icon: 'M1 1" onload="bad<>&\'', badge: 'kn', todayTokens: 1, limits: [] },
    { label: 'Future <agent>', icon: null, badge: 'f<&', todayTokens: 1, limits: [] },
  ] } });
  const html = element('usage').innerHTML;
  assert.match(html, /<path d="M1 1&quot; onload=&quot;bad&lt;&gt;&amp;&#39;"\/><\/svg>Known &lt;agent&gt;<\/div>/);
  assert.match(html, /aria-label="Future &lt;agent&gt;"><div class="uagenthead" role="heading" aria-level="3"><span class="badge" aria-hidden="true">f&lt;&amp;<\/span>Future &lt;agent&gt;<\/div>/);
  assert.doesNotMatch(html, / onload="|<agent>/);
});

test('usage renders weekly-only quotas and leaves unavailable quotas absent', () => {
  const { element, pushState } = popoverPage();
  pushState({ settings: { usageEnabled: true }, usage: { todayTokens: 1200, agents: [
    { harness: 'claude', label: 'Claude', badge: 'cc', todayTokens: 0,
      limits: [{ label: 'Week', usedPercent: 0, reset: null }] },
    { harness: 'codex', label: 'Codex', badge: 'cx', todayTokens: 1200, limits: [] },
    { harness: 'opencode', label: 'opencode', badge: 'oc', todayTokens: 0, limits: [] },
  ] } });
  const html = element('usage').innerHTML;
  assert.match(html, /Claude Week used" aria-valuemin="0" aria-valuemax="100" aria-valuenow="0"/);
  assert.match(visibleLabel(element('usage')), /ClaudeToday ≈ 0 tokensWeek0%/);
  assert.match(visibleLabel(element('usage')), /CodexToday ≈ 1k tokens/);
  assert.equal((html.match(/role="progressbar"/g) || []).length, 1);
  assert.doesNotMatch(html, /5h block|resets|opencode/);
  assert.equal(element('usage').hidden, false);
  pushState({ settings: { usageEnabled: true }, usage: { todayTokens: 0, agents: [] } });
  assert.equal(element('usage').hidden, true);
  assert.equal(element('usage').innerHTML, '');
});

test('disabled usage stays hidden even with a cached snapshot', () => {
  const { element, pushState, click, listeners } = popoverPage();
  const usage = { todayTokens: 100, agents: [
    { harness: 'codex', label: 'Codex', badge: 'cx', todayTokens: 100,
      limits: [{ label: 'Week', usedPercent: 44, reset: 'Fri' }] },
  ] };
  pushState({ settings: { usageEnabled: true }, usage });
  assert.equal(element('usage').hidden, false);
  for (const settings of [{ usageEnabled: false }, {}]) {
    pushState({ settings, usage });
    assert.equal(element('usage').hidden, true);
    assert.equal(element('usage').innerHTML, '');
  }
  pushState({ settings: { usageEnabled: true }, usage });
  click('settingsbtn');
  element('trackusage').checked = false;
  listeners.change({ target: element('trackusage') });
  click('settingsbtn');
  assert.equal(element('usage').hidden, true, 'Disabling hides cached usage before the next host snapshot');
});

test('hiding menu-bar usage preserves popover totals without changing the tracking opt-in', () => {
  const { context, element, click, listeners, commands, pushState } = popoverPage();
  const usage = { todayTokens: 100, agents: [
    { harness: 'codex', label: 'Codex', badge: 'cx', todayTokens: 100,
      limits: [{ label: 'Week', usedPercent: 44, reset: 'Fri' }] },
  ] };
  const settings = { usageEnabled: true, showUsageInMenuBar: true };
  pushState({ settings, usage });
  click('settingsbtn');
  element('showusageinmenubar').checked = false;
  listeners.change({ target: element('showusageinmenubar') });
  assert.deepEqual(commands('set_usage_in_menu_bar'), [{ cmd: 'set_usage_in_menu_bar', enabled: false }]);
  assert.equal(commands('set_usage').length, 0);
  pushState({ settings: { ...settings, showUsageInMenuBar: false }, usage });
  context.window.__configActionResult(null);
  click('settingsbtn');
  assert.equal(element('usage').hidden, false);
  assert.match(visibleLabel(element('usage')), /CodexToday ≈ 100 tokensWeek44%resets FriAll agents today ≈ 100 tokens/);
});

test('Settings toggles the saved Dashboard preference, reflects CLI changes and recovers from save failures', () => {
  const { context, element, click, listeners, commands, messages } = popoverPage();
  const refresh = enabled => context.window.__setState({ aggregate: 'none', sessions: [],
    settings: { showDashboardButton: enabled }, lamp: lamp() });
  assert.equal(element('dashboardbtn').hidden, true, 'Dashboard is opt-in');
  refresh(false);
  click('settingsbtn');
  const toggle = element('showdashboard');
  assert.equal(toggle.checked, false);
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
  assert.ok(!/Troubleshooting/.test(groups.innerHTML), 'Per-lamp Connection details replace a global block');
  assert.ok(!/<[^>]*id="lampdetail"[^>]*role="status"/.test(groups.innerHTML));
  assert.equal(element('lamptransport').textContent, 'Bluetooth');
  assert.equal(element('lampstate-0').textContent, 'Connected');
  assert.equal(element('lampstate-1').textContent, 'Looking for lamp');
  assert.equal(element('lampcontrol').textContent, 'Disable lamps');
  assert.equal(element('lampid-0').textContent, 'home-id');
  assert.equal(element('lampid-1').textContent, 'work-id');
  assert.equal(element('lampdetail').hidden, true, 'Worker detail belongs to the lamp rows');
  assert.ok(!/home-id|work-id|Write acknowledged/.test(element('lampsummary').textContent));
  const summaries = [...list.innerHTML.matchAll(/<summary class="lampheading">([\s\S]*?)<\/summary>/g)];
  assert.equal(summaries.length, 2);
  assert.match(summaries[0][1], /Lamp 1/);
  assert.match(summaries[1][1], /Lamp 2/);
  assert.ok(summaries.every(([, text]) => !/home-id|work-id|Write acknowledged/.test(text)));
  const rows = [...list.innerHTML.matchAll(/<details\b[^>]*>([\s\S]*?)<\/details>/g)];
  rows.forEach(([, html], i) => {
    assert.match(html, /<details class="lamptechnical"><summary>Connection details<\/summary>/);
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
  const { element, listeners, messages, click, update, commands } = lampPage();
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
  update(lamp({ transport: 'usb', present: false, enabled: false, devices: [] }));
  assert.equal(element('lampcontrol').dataset.cmd, 'set_led');
  assert.equal(element('lampcontrol').textContent, 'Enable lamp');
  assert.equal(element('lampcontrol').disabled, true);
  click('lampcontrol');
  assert.equal(commands('set_led').length, 1, 'An absent USB lamp cannot be enabled');
  update(lamp({ transport: 'usb', present: true, enabled: false, devices: [] }));
  assert.equal(element('lampcontrol').disabled, false);
  click('lampcontrol');
  assert.deepEqual(messages.at(-1), { cmd: 'set_led', enabled: true });
});

test('Add lamp works with no saved lamp in USB or Bluetooth mode and respects platform capability', () => {
  for (const transport of ['usb', 'ble']) {
    const { element, update, click, commands } = popoverPage();
    const empty = lamp({ transport, bluetooth_supported: true, enabled: false,
      selected_count: 0, devices: [], state: 'disabled' });
    update(empty);
    click('lampbtn');
    assert.equal(element('lampsetup').hidden, false);
    assert.equal(element('lampadd').textContent, 'Add lamp');
    assert.match(element('groups').innerHTML, /Connect one lamp by USB to add it automatically\./);
    click('lampadd');
    click('lampadd');
    assert.deepEqual(commands('add_lamp'), [{ cmd: 'add_lamp' }]);
    assert.equal(element('lampadd').textContent, 'Adding…');
    update(empty); // A state push that predates the request cannot permit a duplicate.
    click('lampadd');
    assert.equal(commands('add_lamp').length, 1);
    assert.equal(element('lampadd').disabled, true);
    assert.ok(!element('lampbtn').innerHTML.includes('dgreen'));
  }
  for (const bluetooth_supported of [false, undefined]) {
    const { element, update, click, commands } = popoverPage();
    update(lamp({ bluetooth_supported }));
    click('lampbtn');
    assert.equal(element('lampsetup').hidden, true);
    click('lampadd');
    assert.equal(commands('add_lamp').length, 0);
  }
});

test('host setup busy survives reopening and stops lamp mutations and active preview timers', () => {
  const { element, update, click, listeners, commands, advance } = lampPage({ bluetooth_supported: true });
  const slider = element('brightness');
  listeners.pointerdown({ target: slider, pointerId: 1 });
  slider.value = '25';
  listeners.input({ target: slider });
  const previews = commands('preview_brightness').length;
  update(lamp({ bluetooth_supported: true, setup_busy: true }));
  assert.equal(element('lampstate-0').textContent, 'Connected', 'Setup does not invent a connection state');
  click('settingsbtn');
  click('lampbtn');
  assert.equal(element('lampadd').textContent, 'Adding…');
  assert.equal(element('lampsetupresult').textContent, 'Setting up USB lamp…');
  assert.equal(element('lampsetupresult').attributes.role, 'status');
  for (const id of ['lampadd', 'lampforget-0', 'lampname-0', 'lampsave-0', 'lampidentify-0', 'lampcontrol', 'brightness', 'lampbrightness-0']) {
    assert.equal(element(id).disabled, true, id);
  }
  for (const id of ['lampadd', 'lampforget-0', 'lampsave-0', 'lampidentify-0', 'lampcontrol']) click(id);
  listeners.pointerdown({ target: slider, pointerId: 2 });
  listeners.input({ target: slider });
  listeners.change({ target: slider });
  advance(3000);
  assert.equal(commands('preview_brightness').length, previews, 'No queued preview or stationary renewal survives setup');
  for (const cmd of ['add_lamp', 'forget_lamp', 'rename_lamp', 'identify_lamp', 'set_led', 'set_brightness']) {
    assert.equal(commands(cmd).length, 0, cmd);
  }
});

test('Add lamp reports errors with retry and success without claiming a connection', () => {
  const { context, element, update, click, commands } = popoverPage();
  const empty = lamp({ bluetooth_supported: true, enabled: false, state: 'disabled', selected_count: 0, devices: [] });
  update(empty);
  click('lampbtn');
  click('lampadd');
  const error = 'More than one USB lamp <found>: connect only one';
  context.window.__lampSetupResult(error);
  update(empty);
  const result = element('lampsetupresult');
  assert.equal(result.hidden, false);
  assert.equal(result.textContent, error);
  assert.equal(result.innerHTML, '');
  assert.equal(result.attributes.role, 'alert');
  assert.equal(element('lampadd').disabled, false);
  click('lampadd');
  assert.equal(commands('add_lamp').length, 2);
  assert.equal(result.textContent, 'Setting up USB lamp…');
  assert.equal(result.attributes.role, 'status');
  update(lamp({ bluetooth_supported: true, state: 'searching',
    devices: [{ id: 'new-lamp', state: 'searching' }] }));
  context.window.__lampSetupResult(null);
  assert.equal(result.textContent, 'Lamp added.');
  assert.equal(result.attributes.role, 'status');
  assert.equal(element('lampadd').disabled, false);
  assert.equal(element('lampstate-0').textContent, 'Looking for lamp');
  assert.ok(!element('lampbtn').innerHTML.includes('dgreen'));
});

test('saving a name keeps Add lamp visually steady while preventing overlapping setup', () => {
  for (const error of [null, 'Cannot save name']) {
    const { context, element, update, click, commands } = popoverPage();
    const current = lamp({ bluetooth_supported: true, names: { 'saved-lamp': 'Home' } });
    update(current);
    click('lampbtn');
    const add = element('lampadd');
    const appearance = () => [add.textContent, add.disabled];
    const before = appearance();
    element('lampname-0').value = 'Office';
    click('lampsave-0');
    assert.deepEqual(appearance(), before, 'Saving must not dim or relabel Add lamp');
    assert.equal(add.attributes['aria-disabled'], 'true');
    click('lampadd');
    assert.equal(commands('add_lamp').length, 0, 'Setup still waits for the pending save');
    update(current);
    assert.deepEqual(appearance(), before, 'A heartbeat must not flash Add lamp either');
    context.window.__configActionResult(error);
    assert.deepEqual(appearance(), before);
    assert.equal(add.attributes['aria-disabled'], 'false');
    click('lampadd');
    assert.equal(commands('add_lamp').length, 1, 'Setup becomes available after success or failure');
  }
});

test('names render as text and drafts survive heartbeat, focus changes and failed saves', () => {
  const { context, element, update, click, listeners, commands } = popoverPage();
  const devices = [{ id: 'home-id', state: 'searching' }, { id: 'work-id', state: 'searching' }];
  const refresh = names => update(lamp({ enabled: false, selected_count: 2, devices, names, bluetooth_supported: true }));
  refresh({ 'home-id': '<Home & desk>' });
  click('lampbtn');
  const input = element('lampname-0'), list = element('lamplist');
  const builds = list.writes;
  assert.equal(element('lamplabel-0').textContent, '<Home & desk>');
  assert.equal(element('lamplabel-1').textContent, 'Lamp 2');
  assert.ok(!list.innerHTML.includes('<Home & desk>'));
  input.focus();
  input.value = '  Дом 🏡  ';
  listeners.input({ target: input });
  refresh({ 'home-id': 'Changed elsewhere' });
  assert.equal(context.document.activeElement, input);
  assert.equal(input.value, '  Дом 🏡  ');
  context.document.activeElement = null; // Clicking Save moves focus before its click event.
  refresh({ 'home-id': 'Changed elsewhere' });
  assert.equal(input.value, '  Дом 🏡  ');
  click('lampsave-0');
  assert.deepEqual(commands('rename_lamp'), [{ cmd: 'rename_lamp', id: 'home-id', name: 'Дом 🏡' }]);
  assert.equal(element('lamplabel-0').textContent, 'Changed elsewhere', 'No optimistic saved name');
  assert.equal(element('lampsave-0').textContent, 'Saving…');
  assert.equal(element('lampnamesaved-0').hidden, true, 'Wait for a successful host response');
  click('lampsave-1');
  click('lampforget-1');
  click('lampadd');
  assert.equal(commands('rename_lamp').length, 1);
  assert.equal(commands('forget_lamp').length, 0);
  assert.equal(commands('add_lamp').length, 0);
  context.window.__configActionResult('Cannot save name');
  refresh({ 'home-id': 'Changed elsewhere' });
  assert.equal(element('lamperror').textContent, 'Cannot save name');
  assert.equal(element('lampnamesaved-0').hidden, true, 'A failed save cannot show success');
  assert.equal(input.value, '  Дом 🏡  ');
  assert.equal(element('lampsave-0').disabled, false);
  input.focus();
  click('lampsave-0');
  refresh({ 'home-id': 'Дом 🏡' });
  context.window.__configActionResult(null);
  assert.equal(element('lamplabel-0').textContent, 'Дом 🏡');
  assert.equal(input.value, 'Дом 🏡');
  assert.notEqual(context.document.activeElement, input, 'Successful save ends editing');
  assert.equal(element('lampnamesaved-0').textContent, 'Name saved.');
  assert.equal(element('lampnamesaved-0').hidden, false);
  assert.equal(element('lampnamesaved-1').hidden, true, 'Feedback belongs to the renamed lamp');
  assert.match(list.innerHTML, /id="lampnamesaved-0" role="status"/);
  assert.equal(element('lamperror').hidden, true);
  assert.equal(list.writes, builds, 'Names and drafts do not rebuild disclosure controls');
  refresh({ 'home-id': 'Дом 🏡' });
  assert.equal(element('lampnamesaved-0').hidden, false, 'Feedback survives a heartbeat');
  input.focus();
  input.value = 'Next name';
  listeners.input({ target: input });
  assert.equal(element('lampnamesaved-0').hidden, true, 'Editing clears the previous confirmation');
  assert.equal(context.document.activeElement, input);
  assert.equal(input.value, 'Next name');
});

test('Enter in a lamp name saves it like the Save button', () => {
  const { context, element, update, click, listeners, commands } = popoverPage();
  update(lamp({ state: 'searching', devices: [{ id: 'saved-lamp', state: 'searching' }], names: {} }));
  click('lampbtn');
  const input = element('lampname-0');
  input.focus();
  input.value = ' Desk ';
  let prevented = false;
  listeners.keydown({ key: 'Enter', isComposing: true, target: input, preventDefault() {} });
  assert.equal(commands('rename_lamp').length, 0, 'IME composition Enter does not save');
  listeners.keydown({ key: 'Enter', target: input, preventDefault() { prevented = true; } });
  assert.deepEqual(commands('rename_lamp'), [{ cmd: 'rename_lamp', id: 'saved-lamp', name: 'Desk' }]);
  assert.ok(prevented);
  update(lamp({ state: 'searching', devices: [{ id: 'saved-lamp', state: 'searching' }], names: { 'saved-lamp': 'Desk' } }));
  context.window.__configActionResult(null);
  assert.equal(element('lampnamesaved-0').textContent, 'Name saved.');
  assert.equal(element('lampnamesaved-0').hidden, false);
  assert.notEqual(context.document.activeElement, input);
  assert.equal(input.value, 'Desk');
});

test('rename accepts 40 Unicode characters, rejects longer names and resets to automatic with blank input', () => {
  const { context, element, update, click, listeners, commands } = popoverPage();
  const refresh = names => update(lamp({ state: 'searching', devices: [{ id: 'saved-lamp', state: 'searching' }], names }));
  refresh({});
  click('lampbtn');
  const input = element('lampname-0'), name = '🏡'.repeat(40);
  input.value = '  ' + name + '  ';
  listeners.input({ target: input });
  click('lampsave-0');
  assert.deepEqual(commands('rename_lamp').at(-1), { cmd: 'rename_lamp', id: 'saved-lamp', name });
  refresh({ 'saved-lamp': name });
  context.window.__configActionResult(null);
  input.value = name + '🏡';
  listeners.input({ target: input });
  click('lampsave-0');
  assert.equal(commands('rename_lamp').length, 1);
  assert.match(element('lamperror').textContent, /40 characters/);
  input.value = '   ';
  listeners.input({ target: input });
  context.document.activeElement = null;
  click('lampsave-0');
  assert.deepEqual(commands('rename_lamp').at(-1), { cmd: 'rename_lamp', id: 'saved-lamp', name: '' });
  refresh({});
  context.window.__configActionResult(null);
  assert.equal(element('lamplabel-0').textContent, 'Lamp 1');
  assert.equal(input.value, '');
  assert.equal(element('lampnamesaved-0').hidden, false, 'Clearing a name also confirms success');
  assert.equal(commands('set_led').length, 0);
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
    assert.ok(!list.innerHTML.includes(devices[1].id), 'IDs are data, never markup');
    click('lampforget-1');
    assert.deepEqual(commands('forget_lamp'), [{ cmd: 'forget_lamp', id: devices[1].id }]);
    assert.equal(element('lampforget-1').textContent, 'Forgetting…');
    assert.equal(element('lampforget-0').disabled, true);
    assert.equal(element('lampsave-0').disabled, true);
    assert.equal(element('lampid-0').textContent, 'home-id');
    assert.equal(element('lampid-1').textContent, devices[1].id, 'No optimistic removal before config succeeds');
    click('lampforget-0');
    click('lampsave-0');
    click('lampidentify-0');
    assert.equal(commands('forget_lamp').length, 1, 'Only one action can be pending');
    assert.equal(commands('rename_lamp').length, 0);
    assert.equal(commands('identify_lamp').length, 0);
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
  const { context, element, click, update, commands } = lampPage();
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

test('Identify targets only the selected connected lamp, including at zero brightness, without changing preferences', () => {
  const { context, element, update, click, commands } = popoverPage();
  update(lamp({ brightness_percent: 0, selected_count: 2, devices: [
    { id: 'home-id', state: 'connected' }, { id: 'work-id', state: 'connected' },
  ] }));
  click('lampbtn');
  assert.equal(element('lampbrightnesshint-1').hidden, true);
  assert.equal(element('lampidentify-1').disabled, false);
  click('lampidentify-1');
  click('lampidentify-0');
  assert.deepEqual(commands('identify_lamp'), [{ cmd: 'identify_lamp', id: 'work-id' }]);
  assert.equal(element('lampidentify-1').textContent, 'Sending…');
  assert.equal(element('lampsave-0').disabled, true);
  context.window.__configActionResult('Lamp disconnected before the command');
  assert.equal(element('lamperror').textContent, 'Lamp disconnected before the command');
  assert.equal(element('lampidentify-1').disabled, false);
  click('lampidentify-1');
  context.window.__configActionResult(null);
  assert.equal(element('lampidentify-1').textContent, 'Identify', 'Acknowledgement does not claim a visible scan');
  assert.equal(element('lampsave-0').disabled, false, 'No artificial animation lock after the result');
  element('lampname-0').value = 'Home';
  click('lampsave-0');
  click('lampidentify-1');
  assert.equal(commands('identify_lamp').length, 2, 'A pending rename blocks another lamp action');
  assert.equal(element('brightness').value, '0');
  for (const cmd of ['set_brightness', 'preview_brightness', 'set_led']) assert.equal(commands(cmd).length, 0);
});

test('Identify requires that lamp to have a fresh enabled Bluetooth connection', () => {
  const { element, update, click, commands } = lampPage();
  for (const unavailable of [
    { enabled: false }, { state: 'unavailable' }, { state: 'searching' }, { setup_busy: true },
    { devices: [{ id: 'saved-lamp', state: 'searching' }, { id: 'other-lamp', state: 'connected' }], selected_count: 2 },
  ]) {
    update(lamp(unavailable));
    assert.equal(element('lampidentify-0').disabled, true);
    click('lampidentify-0');
  }
  update(lamp({ transport: 'usb' }));
  assert.ok(!element('lamplist').innerHTML.includes('lampidentify'));
  click('lampidentify-0'); // A stale control cannot bypass the transport guard.
  assert.equal(commands('identify_lamp').length, 0);
});

test('per-lamp brightness targets one slider through throttle, heartbeat, stationary hold and release', () => {
  const { context, element, update, click, listeners, commands, advance } = popoverPage();
  const current = lamp({ selected_count: 2, brightness_percent: 100,
    brightness_by_id: { home: 20, work: 80 },
    devices: [{ id: 'home', state: 'connected' }, { id: 'work', state: 'connected' }] });
  update(current);
  click('lampbtn');
  const slider = element('lampbrightness-1'), listBuilds = element('lamplist').writes;
  assert.equal(element('lampbrightness-0').value, '20');
  assert.equal(slider.value, '80');
  listeners.pointerdown({ target: slider, pointerId: 8 });
  for (const value of ['70', '60']) { slider.value = value; listeners.input({ target: slider }); }
  advance(110);
  assert.deepEqual(commands('preview_brightness').at(-1), { cmd: 'preview_brightness', id: 'work', percent: 60 });
  assert.equal(element('lampbrightnessvalue-1').textContent, '60%');
  context.document.activeElement = null;
  listeners.focusout({ target: slider });
  update(current);
  assert.equal(slider.value, '60', 'The held target survives focus loss and host refresh');
  assert.equal(element('lampbrightness-0').value, '20');
  assert.equal(element('brightness').value, '100');
  const before = commands('preview_brightness').length;
  advance(1100);
  assert.ok(commands('preview_brightness').length >= before + 2);
  assert.ok(commands('preview_brightness').every(message => message.id === 'work'));
  listeners.change({ target: slider });
  assert.deepEqual(commands('set_lamp_brightness'), [{ cmd: 'set_lamp_brightness', id: 'work', percent: 60 }]);
  listeners.pointerup({ pointerId: 8 });
  const sent = commands('preview_brightness').length;
  advance(3000);
  assert.equal(commands('preview_brightness').length, sent);
  assert.equal(element('lamplist').writes, listBuilds);
  assert.equal(commands('set_brightness').length, 0);
  assert.equal(commands('set_led').length, 0);
  // Removing the active row must stop its timer, not retarget an index or broadcast.
  listeners.pointerdown({ target: slider, pointerId: 9 });
  update(lamp({ devices: [{ id: 'home', state: 'connected' }] }));
  const afterRemoval = commands('preview_brightness').length;
  listeners.change({ target: slider });
  advance(3000);
  assert.equal(commands('preview_brightness').length, afterRemoval);
  assert.equal(commands('set_lamp_brightness').length, 1);
});

test('offline per-lamp saves never broadcast and All lamps resets effective values only from host state', () => {
  for (const enabled of [true, false]) {
    const { context, element, update, click, listeners, commands } = popoverPage();
    const devices = [{ id: 'home', state: 'connected' }, { id: 'work', state: 'searching' }];
    const refresh = (brightness_by_id, brightness_percent = 100) => update(lamp({ enabled,
      selected_count: 2, devices, brightness_by_id, brightness_percent }));
    refresh({ home: 20, work: 80 });
    click('lampbtn');
    const slider = element('lampbrightness-1');
    slider.value = '30';
    context.document.activeElement = slider;
    listeners.input({ target: slider });
    refresh({ home: 20, work: 80 });
    assert.equal(slider.value, '30', 'A focused per-lamp keyboard edit survives host refresh');
    listeners.change({ target: slider });
    assert.deepEqual(commands('set_lamp_brightness').at(-1), { cmd: 'set_lamp_brightness', id: 'work', percent: 30 });
    assert.equal(commands('preview_brightness').length, 0, 'A connected neighbor is not the preview target');
    context.window.__configActionResult('Could not save brightness');
    assert.equal(element('lamperror').textContent, 'Could not save brightness');
    assert.equal(slider.disabled, false, 'Failure leaves the control available for retry');
    slider.value = '35';
    listeners.change({ target: slider });
    refresh({ home: 20, work: 35 });
    context.window.__configActionResult(null);
    assert.equal(element('lampbrightness-0').value, '20');
    assert.equal(slider.value, '35');
    context.document.activeElement = null;
    const all = element('brightness');
    assert.match(element('groups').innerHTML, /for="brightness">All lamps/);
    assert.match(element('brightnesshint').textContent, /Replaces individual brightness settings/);
    all.value = '45';
    listeners.change({ target: all });
    assert.deepEqual(commands('set_brightness'), [{ cmd: 'set_brightness', percent: 45 }]);
    assert.equal(element('lampbrightness-0').value, '20', 'No optimistic global reset');
    refresh({}, 45);
    context.window.__configActionResult(null);
    assert.equal(element('lampbrightness-0').value, '45');
    assert.equal(slider.value, '45');
    assert.equal(commands('set_led').length, 0);
  }
});

test('brightness saves while absent without enabling lamps and survives focused edits during refresh', () => {
  const { context, element, listeners, click, update, commands } = popoverPage();
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
  assert.equal(element('lampcontrol').dataset.enabled, 'true', 'Saving does not enable the lamp');
});

test('brightness preview throttles live input to the latest value and commits only on change', () => {
  const { element, listeners, commands, advance } = lampPage();
  assert.equal(element('brightnesshint').textContent, 'Replaces individual brightness settings.');
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
  const { context, element, listeners, update, commands, advance } = lampPage();
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
  for (const sliderId of ['brightness', 'lampbrightness-0']) {
    for (const end of ['blur', 'hidden', 'Escape', 'Back', 'reset']) {
      const { context, element, listeners, windowListeners, click, commands, advance, pushState } = lampPage();
      const slider = element(sliderId);
      listeners.pointerdown({ target: slider, pointerId: 1 });
      slider.value = '23';
      listeners.input({ target: slider });
      if (end === 'blur') windowListeners.blur();
      else if (end === 'hidden') { context.document.hidden = true; listeners.visibilitychange(); }
      else if (end === 'Escape') listeners.keydown({ key: 'Escape' });
      else if (end === 'reset') pushState({}, true);
      else click('settingsbtn');
      assert.equal(commands('preview_brightness').at(-1).percent, 23, end);
      assert.equal(commands('preview_brightness').at(-1).id, sliderId === 'brightness' ? undefined : 'saved-lamp');
      const sent = commands('preview_brightness').length;
      advance(3000);
      assert.equal(commands('preview_brightness').length, sent, end);
      assert.equal(commands('set_brightness').length, 0);
      assert.equal(commands('set_led').length, 0);
    }
  }
});

for (const { sliderId, command, target } of [
  { sliderId: 'brightness', command: 'set_brightness', target: {} },
  { sliderId: 'lampbrightness-0', command: 'set_lamp_brightness', target: { id: 'saved-lamp' } },
]) {
  test(`${sliderId} accepts whole percentages, including zero, without pointer renewal`, () => {
    const { element, listeners, commands, advance } = lampPage();
    const slider = element(sliderId);
    for (const invalid of ['101', '-1', '1.5', '', 'invalid']) {
      slider.value = invalid;
      listeners.input({ target: slider });
      listeners.change({ target: slider });
    }
    assert.equal(commands('preview_brightness').length, 0);
    assert.equal(commands(command).length, 0);

    for (const percent of [0, 100]) {
      slider.value = String(percent);
      listeners.input({ target: slider });
      listeners.change({ target: slider });
      assert.deepEqual(commands('preview_brightness').at(-1), { cmd: 'preview_brightness', ...target, percent });
      assert.deepEqual(commands(command).at(-1), { cmd: command, ...target, percent });
    }
    const sent = commands('preview_brightness').length;
    advance(3000);
    assert.equal(commands('preview_brightness').length, sent, 'Keyboard edits do not start pointer renewal');
  });
}

test('preview requires an enabled connected wireless lamp but saving does not', () => {
  const { element, listeners, update, commands, advance } = lampPage();
  const slider = element('brightness');
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
    assert.equal(commands('preview_brightness').length, 0, JSON.stringify(unavailable));
    assert.equal(commands('set_brightness').at(-1).percent, 45, 'Unavailable lamps still accept saved preferences');
  }
  assert.equal(commands('set_led').length, 0);
});

test('firmware section states: facts per line, a button only for a board on USB', () => {
  const { context, element, update, click, commands, messages } = lampPage();
  const hint = () => element('lampfirmwarehint').textContent;
  const note = () => element('lampfirmwarenote');
  const button = () => element('lampflash');
  assert.equal(element('lampfirmware').hidden, true, 'No bundled images, no section');
  assert.equal(element('lampfirmwarelabel').hidden, true);
  assert.match(element('groups').innerHTML, /<div class="grplabel" id="lampfirmwarelabel" hidden>Firmware<\/div><div class="lampsetup" id="lampfirmware" hidden>/,
    'The heading sits beside the section, not inside its padding');
  click('lampflash');
  assert.equal(commands('update_firmware').length, 0);
  const firmware = { available: '1.2.0', busy: false, progress: 0 };
  const names = { 'saved-lamp': 'Home', 'office-id': 'Office' };

  // Bluetooth only: what each known lamp runs, one version per sentence, no button.
  update(lamp({ firmware, names, devices: [{ id: 'saved-lamp', state: 'connected', firmware: '1.1.0' }] }));
  assert.equal(element('lampfirmware').hidden, false);
  assert.equal(hint(), 'Lamp “Home” runs firmware 1.1.0.');
  assert.equal(note().textContent, 'Connect it by USB to update to 1.2.0.');
  assert.equal(button().hidden, true, 'Nothing on USB, nothing to press');
  assert.equal(element('lampfirmware-0').textContent, '1.1.0');
  update(lamp({ firmware, names, selected_count: 2, devices: [
    { id: 'saved-lamp', state: 'searching', firmware: '1.1.0' }, { id: 'office-id', state: 'connected', firmware: '1.0.5' }] }));
  assert.equal(hint(), 'Lamp “Home” runs firmware 1.1.0. Lamp “Office” runs firmware 1.0.5.', 'An offline lamp keeps its last known version');
  assert.equal(note().textContent, 'Connect a lamp by USB to update to 1.2.0.');
  update(lamp({ firmware, names, selected_count: 2, devices: [
    { id: 'saved-lamp', state: 'connected', firmware: '1.1.0' }, { id: 'office-id', state: 'searching', firmware: '1.1.0' }] }));
  assert.equal(hint(), 'Lamp “Home” and Lamp “Office” run firmware 1.1.0.', 'Same version, one sentence');
  update(lamp({ firmware, devices: [{ id: 'saved-lamp', state: 'connected', firmware: '1.2.0' }] }));
  assert.equal(hint(), 'The lamp runs firmware 1.2.0, the latest.');
  assert.equal(note().hidden, true);
  update(lamp({ firmware, devices: [{ id: 'saved-lamp', state: 'connected' }] }));
  assert.equal(hint(), 'Firmware 1.2.0 is available.');
  assert.equal(note().textContent, 'Connect a lamp by USB to check or update it.');
  assert.match(element('lampfirmware-0').textContent, /^Unknown/);

  // A board on USB: probing, blank, current, outdated, unsaved.
  update(lamp({ firmware, usb_present: true }));
  assert.equal(hint(), 'Checking the lamp on USB…');
  assert.equal(button().hidden, true);
  update(lamp({ firmware, usb_present: true, usb_lamp: { id: null, version: null, broglow: false } }));
  assert.equal(hint(), 'No BroGlow reply from the USB device.');
  assert.equal(note().textContent, 'Flash new board writes firmware 1.2.0 to a blank ESP32-C3.');
  assert.equal(button().textContent, 'Flash new board');
  assert.equal(button().disabled, false);
  click('lampflash');
  assert.deepEqual(commands('update_firmware'), [{ cmd: 'update_firmware', force: true }], 'A blank board is written only with force');
  context.window.__firmwareResult('Flashed and verified firmware 1.2.0.', null);
  messages.length = 0;
  update(lamp({ firmware, usb_present: true, names, usb_lamp: { id: 'saved-lamp', version: '1.2.0', broglow: true } }));
  assert.equal(hint(), 'Lamp “Home” on USB runs firmware 1.2.0, the latest.');
  assert.equal(button().hidden, true, 'Nothing to write');
  click('lampflash');
  assert.equal(commands('update_firmware').length, 0);
  update(lamp({ firmware, usb_present: true, names, usb_lamp: { id: 'saved-lamp', version: '1.1.0', broglow: true } }));
  assert.equal(hint(), 'Lamp “Home” on USB runs firmware 1.1.0.');
  assert.equal(note().hidden, true);
  assert.equal(button().hidden, false);
  assert.equal(button().disabled, false);
  assert.equal(button().textContent, 'Update to 1.2.0');
  update(lamp({ firmware, usb_present: true, usb_lamp: { id: 'other-lamp', version: '1.1.0', broglow: true } }));
  assert.equal(hint(), 'The lamp on USB runs firmware 1.1.0.', 'A board not saved here, say a friend\'s lamp, still updates');
  update(lamp({ firmware, usb_present: true, usb_lamp: { id: null, version: null, broglow: true } }));
  assert.equal(hint(), 'The lamp on USB runs firmware without a version.');
  update(lamp({ firmware, usb_present: true, transport: 'usb', usb_lamp: null }));
  assert.equal(hint(), 'Firmware 1.2.0 is ready for the lamp on USB.', 'USB mirroring holds the cable: no probe, the write pauses it');
  assert.equal(button().disabled, false);

  // Writing: progress, blocked controls, outcome, retry.
  const usbLamp = { id: 'saved-lamp', version: '1.1.0', broglow: true };
  update(lamp({ firmware, usb_present: true, usb_lamp: usbLamp }));
  click('lampflash');
  click('lampflash');
  assert.deepEqual(commands('update_firmware'), [{ cmd: 'update_firmware' }]);
  assert.equal(button().textContent, 'Updating…');
  assert.equal(hint(), 'Writing firmware 1.2.0… 0%');
  for (const id of ['lampflash', 'lampadd', 'lampidentify-0', 'lampcontrol', 'brightness', 'lampforget-0']) {
    assert.equal(element(id).disabled, true, id);
  }
  assert.equal(element('lampadd').textContent, 'Add lamp', 'A firmware write is not USB setup');
  for (const id of ['lampadd', 'lampidentify-0', 'lampcontrol']) click(id);
  for (const cmd of ['add_lamp', 'identify_lamp', 'set_led']) assert.equal(commands(cmd).length, 0, cmd);
  context.window.__firmwareProgress(42);
  assert.equal(hint(), 'Writing firmware 1.2.0… 42%');
  update(lamp({ firmware: { ...firmware, busy: true, progress: 60 }, usb_present: true, usb_lamp: usbLamp }));
  assert.equal(hint(), 'Writing firmware 1.2.0… 60%');
  context.window.__firmwareResult('Flashed and verified firmware 1.2.0.', null);
  update(lamp({ firmware, usb_present: true, usb_lamp: { ...usbLamp, version: '1.2.0' }, devices: [{ id: 'saved-lamp', state: 'connected', firmware: '1.2.0' }] }));
  const result = element('lampfirmwareresult');
  assert.equal(result.textContent, 'Flashed and verified firmware 1.2.0.');
  assert.equal(result.attributes.role, 'status');
  assert.equal(result.hidden, false);
  assert.equal(button().hidden, true, 'Current after the write');
  update(lamp({ firmware, usb_present: true, usb_lamp: usbLamp }));
  click('lampflash');
  assert.equal(result.hidden, true, 'A new write clears the old outcome');
  context.window.__firmwareResult('', 'Opening USB lamp failed');
  update(lamp({ firmware, usb_present: true, usb_lamp: usbLamp }));
  assert.equal(result.textContent, 'Opening USB lamp failed');
  assert.equal(result.attributes.role, 'alert');
  assert.equal(result.className, 'error');
  assert.equal(commands('update_firmware').length, 2);
});

test('Settings shows the app version and a quiet update check with Homebrew only where it applies', () => {
  const { context, element, click, commands, listeners } = popoverPage();
  const refresh = (app, settings = {}) => context.window.__setState({ aggregate: 'none', sessions: [], settings, lamp: lamp(), app });
  refresh({ version: '1.1.0', update: { checking: false, checked: false, latest: null, available: false, url: null, error: null, homebrew: true, upgrading: false } });
  click('settingsbtn');
  const html = element('groups').innerHTML;
  assert.match(html, /<div class="grplabel">Updates<\/div><div class="optrow static">/, 'A named group; the version row is not a control');
  assert.match(html, /<label class="optrow" for="checkupdatesonstartup" title="One request to GitHub[^"]*Never a notification\."/, 'The long explanation lives in the tooltip');
  assert.match(html, /id="checkupdatesonstartuphint">Checks GitHub releases\.</);
  assert.match(html, /<div class="lampactions"><button[^>]*id="upgradeapp"[^>]*>Update via Homebrew<\/button><button[^>]*id="releasenotes"[^>]*>Release notes<\/button><button[^>]*id="checkupdates"/,
    'The primary action comes first in one row');
  assert.equal(element('appversion').textContent, 'BroGlow 1.1.0');
  assert.equal(element('updatehint').textContent, 'Not checked yet.');
  assert.equal(element('checkupdatesonstartup').checked, true, 'Startup check defaults on');
  assert.equal(element('releasenotes').hidden, true);
  assert.equal(element('upgradeapp').hidden, true);
  click('checkupdates');
  assert.deepEqual(commands('check_updates'), [{ cmd: 'check_updates' }]);
  refresh({ version: '1.1.0', update: { checking: true, checked: false, latest: null, available: false, url: null, error: null, homebrew: true, upgrading: false } });
  assert.equal(element('updatehint').textContent, 'Checking for updates…');
  assert.equal(element('checkupdates').disabled, true);
  refresh({ version: '1.1.0', update: { checking: false, checked: true, latest: null, available: false, url: null, error: 'curl exited with 6', homebrew: true, upgrading: false } });
  assert.equal(element('updatehint').textContent, 'Could not reach GitHub; try again later.', 'Failures stay quiet and generic');
  refresh({ version: '1.1.0', update: { checking: false, checked: true, latest: '1.1.0', available: false, url: 'https://github.com/BroGlow/broglow/releases/tag/v1.1.0', error: null, homebrew: true, upgrading: false } });
  assert.equal(element('updatehint').textContent, 'You have the latest version.');
  assert.equal(element('upgradeapp').hidden, true);
  const available = { checking: false, checked: true, latest: '1.2.0', available: true, url: 'https://github.com/BroGlow/broglow/releases/tag/v1.2.0', error: null, homebrew: true, upgrading: false };
  refresh({ version: '1.1.0', update: available });
  assert.equal(element('updatehint').textContent, 'BroGlow 1.2.0 is available.');
  assert.equal(element('releasenotes').hidden, false);
  assert.equal(element('upgradeapp').hidden, false);
  assert.equal(element('checkupdates').hidden, true, 'Two buttons when an update is available');
  click('releasenotes');
  click('upgradeapp');
  assert.deepEqual(commands('open_release_notes'), [{ cmd: 'open_release_notes' }]);
  assert.deepEqual(commands('upgrade_app'), [{ cmd: 'upgrade_app' }]);
  refresh({ version: '1.1.0', update: { ...available, upgrading: true } });
  assert.match(element('updatehint').textContent, /Updating via Homebrew/);
  assert.equal(element('upgradeapp').disabled, true);
  refresh({ version: '1.1.0', update: { ...available, homebrew: false } });
  assert.equal(element('updatehint').textContent, 'BroGlow 1.2.0 is available. Download it from the release page.');
  assert.equal(element('upgradeapp').hidden, true, 'No Homebrew, no upgrade button');
  assert.equal(element('releasenotes').hidden, false);
  refresh({ version: '1.1.0', update: { ...available, available: false, latest: '1.1.0' } });
  assert.equal(element('checkupdates').hidden, false, 'Up to date: the check button is back');
  const toggle = element('checkupdatesonstartup');
  toggle.checked = false;
  listeners.change({ target: toggle });
  assert.deepEqual(commands('set_check_updates_on_startup'), [{ cmd: 'set_check_updates_on_startup', enabled: false }]);
  assert.ok(commands('set_usage').length === 0 && commands('set_led').length === 0, 'Nothing else moved');
});
