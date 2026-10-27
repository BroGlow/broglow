const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { resolve } = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = readFileSync(resolve(__dirname, 'app-preview.js'), 'utf8');
const popover = readFileSync(resolve(__dirname, '../assets/popover.html'), 'utf8');
const renderer = readFileSync(resolve(__dirname, '../assets/popover/popover.js'), 'utf8');
const popoverDocument = require('../assets/popover/document.js');
class DetailEvent extends Event {
  constructor(type, options) { super(type); this.detail = options?.detail; }
}

async function preview({ fail = false } = {}) {
  const browser = new EventTarget();
  const posted = [], events = [], attributes = {};
  const frame = { dataset: {}, style: {}, parentElement: { style: { setProperty() {} } },
    contentWindow: { postMessage: (...message) => posted.push(message) },
    setAttribute: (name, value) => { attributes[name] = value; } };
  browser.BroGlowPopoverDocument = popoverDocument;
  browser.location = { origin: 'http://localhost:8766' };
  for (const type of ['broglow:open-session', 'broglow:app-error', 'broglow:app-resize', 'broglow:app-aggregate', 'broglow:app-activate', 'broglow:app-usage']) {
    browser.addEventListener(type, event => events.push({ type, detail: event.detail }));
  }
  vm.runInNewContext(source, { window: browser, document: {
    baseURI: 'http://localhost:8766/site/', body: { dataset: {} },
    querySelector: () => frame }, CustomEvent: DetailEvent, URL, Uint8Array, btoa,
    crypto: { randomUUID: () => 'test-channel-token' }, fetch: async url => ({
      ok: !fail, text: async () => readFileSync(resolve(__dirname, '..', new URL(url).pathname.replace(/^\//, '')), 'utf8'), arrayBuffer: async () => {
        const bytes = readFileSync(resolve(__dirname, '../assets/icons', new URL(url).pathname.split('/').pop()));
        return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
      },
    }) });
  await new Promise(resolve => setImmediate(resolve));
  const message = (data, changes = {}) => {
    const event = new Event('message');
    Object.assign(event, { source: frame.contentWindow, origin: 'null', data: { token: 'test-channel-token', ...data }, ...changes });
    browser.dispatchEvent(event);
  };
  return { browser, frame, posted, events, attributes, message };
}

function childPage(html, { now = Date.now() } = {}) {
  const elements = new Map(), sent = [], listeners = {}, documentListeners = {};
  const element = id => {
    if (!elements.has(id)) elements.set(id, { id, dataset: {}, offsetHeight: 280, value: '100',
      textContent: '', hidden: false, innerHTML: '', setAttribute() {}, matches() { return false; } });
    return elements.get(id);
  };
  const parent = { postMessage: message => sent.push(message) };
  const window = { parent, devicePixelRatio: 1, addEventListener: (type, fn) => { listeners[type] = fn; } };
  class DemoDate extends Date {
    constructor(...args) { super(...(args.length ? args : [now])); }
    static now() { return now; }
  }
  const click = target => {
    for (const callback of documentListeners.click ?? []) callback({ target });
  };
  const context = { window, Date: DemoDate, document: { activeElement: null, hidden: false, getElementById: element,
    addEventListener(type, callback) { (documentListeners[type] ??= []).push(callback); } }, setInterval() {}, clearInterval() {}, setTimeout() {}, clearTimeout() {} };
  for (const script of html.matchAll(/<script>([\s\S]*?)<\/script>/g)) vm.runInNewContext(script[1], context);
  return { window, parent, element, sent, listeners,
    advance(milliseconds) { now += milliseconds; },
    clickLocal(view) {
      const target = { dataset: { local: view }, closest(selector) { return selector === '[data-local]' ? this : null; } };
      click(target);
    },
    clickCommand(cmd, enabled) {
      const target = { dataset: { cmd, enabled: String(enabled) },
        closest(selector) { return selector === '[data-cmd]' ? this : null; } };
      click(target);
    },
    activate(type) {
      const event = { target: element('card') };
      for (const callback of documentListeners[type] ?? []) callback(event);
    } };
}

test('embeds the production renderer with native PNG assets and a scripts-only sandbox', async () => {
  const demo = await preview();
  assert.equal(demo.attributes.sandbox, 'allow-scripts');
  assert.ok(demo.frame.srcdoc.includes('width: 338px;'));
  const originalScript = renderer.replace(/__ICON_[A-Z]+__/g, 'ICON');
  const renderedScript = [...demo.frame.srcdoc.matchAll(/<script>([\s\S]*?)<\/script>/g)].at(-1)[1]
    .replace(/data:image\/png;base64,[A-Za-z0-9+/=]+/g, 'ICON');
  assert.equal(renderedScript, originalScript, 'Only the native icon placeholders change in production JavaScript');
  assert.doesNotMatch(demo.frame.srcdoc, /__[A-Z_]+__/);
  assert.match(demo.frame.srcdoc, /data:image\/png;base64,/);
  const child = childPage(demo.frame.srcdoc);
  assert.equal(child.element('aggline').textContent, '3 live sessions');
  assert.match(child.element('groups').innerHTML, /Test the checkout flow/);
  const sessionText = child.element('groups').innerHTML.replace(/<svg\b[\s\S]*?<\/svg>/g, '').replace(/<[^>]*>/g, '');
  assert.doesNotMatch(sessionText, /Claude|Codex/);
  assert.ok(child.sent.some(message => message.type === 'broglow:app-ready'));
});

test('validates iframe messages and forwards state, sizing, and session clicks', async () => {
  const demo = await preview();
  demo.message({ type: 'broglow:open-session', session: 'codex' }, { source: {} });
  demo.message({ type: 'broglow:open-session', session: 'codex' }, { origin: 'https://foreign.example' });
  demo.message({ type: 'broglow:open-session', session: 'codex', token: 'wrong' });
  assert.equal(demo.events.length, 0);
  demo.browser.dispatchEvent(new DetailEvent('broglow:app-state', { detail: { signal: 'red' } }));
  demo.message({ type: 'broglow:app-ready' });
  assert.equal(demo.posted.at(-1)[0].signal, 'red', 'A state change during loading survives');
  demo.message({ type: 'broglow:app-resize', height: 300 });
  assert.equal(demo.frame.style.height, '300px');
  demo.message({ type: 'broglow:open-session', session: 'codex' });
  assert.equal(demo.events.at(-1).detail.session, 'codex');
  demo.message({ type: 'broglow:open-session', session: 'foreign' });
  assert.equal(demo.events.length, 2);
});

test('real renderer follows validated synthetic states and native actions stay inside the demo', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  const send = (signal, changes = {}) => child.listeners.message({ source: child.parent,
    origin: 'http://localhost:8766', data: { token: 'test-channel-token', type: 'broglow:app-state', signal }, ...changes });
  send('red', { source: {} });
  assert.doesNotMatch(child.element('groups').innerHTML, /Needs help/);
  send('red');
  assert.match(child.element('groups').innerHTML, /Needs help/);
  assert.equal(child.element('aggline').textContent, '3 live sessions · 1 needs help');
  child.window.ipc.postMessage(JSON.stringify({ cmd: 'focus', id: 'codex' }));
  assert.equal(child.sent.at(-1).session, 'codex');
  send('yellow');
  assert.match(child.element('groups').innerHTML, /Paused/);
  child.window.ipc.postMessage(JSON.stringify({ cmd: 'clear_paused' }));
  assert.equal(child.element('aggline').textContent, '2 live sessions');
  child.window.ipc.postMessage(JSON.stringify({ cmd: 'quit' }));
  assert.equal(child.sent.at(-1).action, 'quit');
});

test('Show agent names changes the native session metadata in both directions', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  const sessionText = () => child.element('groups').innerHTML
    .replace(/<svg\b[\s\S]*?<\/svg>/g, '').replace(/<[^>]*>/g, '');
  const command = enabled => child.window.ipc.postMessage(JSON.stringify({ cmd: 'set_agent_names', enabled }));
  for (const name of ['Codex', 'Claude', 'opencode']) assert.ok(!sessionText().includes(name));
  command(true);
  for (const name of ['Codex', 'Claude', 'opencode']) {
    assert.ok(sessionText().includes(name), `${name} should appear beside its session when names are enabled`);
  }
  command(false);
  for (const name of ['Codex', 'Claude', 'opencode']) {
    assert.ok(!sessionText().includes(name), `${name} should disappear when names are disabled`);
  }
  assert.match(sessionText(), /Test the checkout flow/);
  assert.match(sessionText(), /shopfront/);
});

test('dashboard visibility and grouping update the native renderer and survive scenario changes', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  const command = message => child.window.ipc.postMessage(JSON.stringify(message));
  assert.equal(child.element('dashboardbtn').hidden, false);
  command({ cmd: 'set_dashboard_button', enabled: false });
  assert.equal(child.element('dashboardbtn').hidden, true);
  command({ cmd: 'set_group_by_project', enabled: true });
  assert.match(child.element('groups').innerHTML, /By project/);
  assert.match(child.element('groups').innerHTML, /class="projhead"/);
  child.listeners.message({ source: child.parent, origin: 'http://localhost:8766',
    data: { token: 'test-channel-token', type: 'broglow:app-state', signal: 'yellow' } });
  assert.equal(child.element('dashboardbtn').hidden, true);
  assert.match(child.element('groups').innerHTML, /By project/);
  command({ cmd: 'set_group_by_project', enabled: false });
  assert.doesNotMatch(child.element('groups').innerHTML, /class="projhead"/);
  assert.match(child.element('groups').innerHTML, />Paused</);
  command({ cmd: 'set_dashboard_button', enabled: true });
  assert.equal(child.element('dashboardbtn').hidden, false);
});

test('Track usage shows illustrative usage in sessions and removes it when disabled', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  const command = enabled => child.window.ipc.postMessage(JSON.stringify({ cmd: 'set_usage', enabled }));
  assert.equal(child.element('usage').hidden, true);
  child.clickLocal('settings');
  command(true);
  assert.equal(child.element('trackusage').checked, true);
  assert.equal(child.element('usage').hidden, true, 'usage belongs to the sessions view');
  child.clickLocal('sessions');
  assert.equal(child.element('usage').hidden, false);
  assert.match(child.element('usage').innerHTML, /62%/);
  assert.match(child.element('usage').innerHTML, /31%/);
  assert.match(child.element('usage').innerHTML, /1\.3M tokens/);
  const usageText = child.element('usage').innerHTML.replace(/<[^>]*>/g, '');
  assert.match(usageText, /ClaudeToday ≈ 1\.2M tokens/);
  assert.match(usageText, /CodexToday ≈ 60k tokens/);
  assert.match(usageText, /opencodeToday ≈ 12k tokens/);
  assert.match(usageText, /12%/);
  assert.match(usageText, /44%/);
  assert.match(child.element('usage').innerHTML, /4:00 PM/);
  assert.match(child.element('usage').innerHTML, /Fri/);
  command(false);
  assert.equal(child.element('usage').hidden, true);
  assert.equal(child.element('usage').innerHTML, '');
});

test('menu-bar usage is independent of tracking and retains the saved display choice', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  const command = (cmd, enabled) => child.window.ipc.postMessage(JSON.stringify({ cmd, enabled }));
  const readout = () => child.sent.filter(message => message.type === 'broglow:app-usage').at(-1).usage;
  child.clickLocal('settings');
  assert.equal(child.element('showusageinmenubar').checked, true);
  assert.equal(child.element('showusageinmenubar').disabled, true);
  assert.equal(readout(), null);
  command('set_usage', true);
  assert.equal(child.element('showusageinmenubar').disabled, false);
  assert.equal(readout().agents.length, 3);
  command('set_usage_in_menu_bar', false);
  assert.equal(child.element('trackusage').checked, true);
  assert.equal(child.element('showusageinmenubar').checked, false);
  assert.equal(readout(), null);
  child.clickLocal('sessions');
  assert.equal(child.element('usage').hidden, false, 'hiding menu-bar usage preserves the popover totals');
  for (const enabled of [false, true]) command('set_usage', enabled);
  assert.equal(readout(), null, 'tracking does not reset the saved readout preference');
  command('set_usage_in_menu_bar', true);
  assert.equal(readout().todayTokens, 1312400);
  command('set_usage', false);
  assert.equal(readout(), null);
  assert.equal(child.element('usage').hidden, true);
});

test('only authenticated, valid usage snapshots reach the landing', async () => {
  const demo = await preview();
  const usage = { todayTokens: 60, agents: [{ label: 'Codex', badge: 'cx', limits: [{ usedPercent: 12 }] }] };
  demo.message({ type: 'broglow:app-usage', usage }, { source: {} });
  demo.message({ type: 'broglow:app-usage', usage }, { origin: 'https://foreign.example' });
  demo.message({ type: 'broglow:app-usage', usage, token: 'wrong' });
  demo.message({ type: 'broglow:app-usage', usage: { todayTokens: -1, agents: [] } });
  demo.message({ type: 'broglow:app-usage', usage: { ...usage, agents: [null] } });
  demo.message({ type: 'broglow:app-usage', usage: { ...usage, agents: [{ ...usage.agents[0], limits: [null] }] } });
  demo.message({ type: 'broglow:app-usage', usage: { ...usage, agents: [{ ...usage.agents[0], limits: [{ usedPercent: Infinity }] }] } });
  assert.equal(demo.events.length, 0);
  demo.message({ type: 'broglow:app-usage', usage });
  assert.equal(demo.events.at(-1).detail.usage.todayTokens, 60);
  demo.message({ type: 'broglow:app-usage', usage: null });
  assert.equal(demo.events.at(-1).detail.usage, null);
});

test('Return to chats resets navigation only after an enabled closed interval', async () => {
  const child = childPage((await preview()).frame.srcdoc, { now: Date.parse('2026-10-22T10:00:00Z') });
  const command = message => child.window.ipc.postMessage(JSON.stringify(message));
  const visibility = open => child.listeners.message({ source: child.parent, origin: 'http://localhost:8766',
    data: { token: 'test-channel-token', type: 'broglow:app-visibility', open } });
  command({ cmd: 'set_agent_names', enabled: true });
  command({ cmd: 'set_group_by_project', enabled: true });
  command({ cmd: 'set_popover_reset', enabled: true });
  command({ cmd: 'set_popover_reset_minutes', minutes: 1 });
  command({ cmd: 'clear_session', id: 'codex' });
  child.clickLocal('settings');
  visibility(false);
  child.advance(59999);
  visibility(true);
  assert.equal(child.element('groups').dataset.view, 'settings', 'short closes retain the chosen view');
  assert.equal(child.element('showagentnames').checked, true);
  assert.equal(child.element('groupbyproject').checked, true);
  visibility(false);
  child.advance(60000);
  visibility(true);
  assert.equal(child.element('groups').dataset.view, 'sessions');
  assert.match(child.element('groups').innerHTML, /By project/);
  assert.equal(child.element('aggline').textContent, '2 live sessions');
  assert.doesNotMatch(child.element('groups').innerHTML, /Test the checkout flow/);
  child.clickLocal('settings');
  visibility(true);
  assert.equal(child.element('groups').dataset.view, 'settings', 'opening an already open panel does not reset it');
  command({ cmd: 'set_popover_reset', enabled: false });
  visibility(false);
  child.advance(120000);
  visibility(true);
  assert.equal(child.element('groups').dataset.view, 'settings', 'disabled reset preserves navigation');
});

test('only authenticated boolean visibility messages affect the reset interval', async () => {
  const child = childPage((await preview()).frame.srcdoc, { now: Date.parse('2026-10-22T10:00:00Z') });
  const command = message => child.window.ipc.postMessage(JSON.stringify(message));
  const visibility = (open, changes = {}, data = {}) => child.listeners.message({ source: child.parent,
    origin: 'http://localhost:8766', data: { token: 'test-channel-token', type: 'broglow:app-visibility', open, ...data }, ...changes });
  command({ cmd: 'set_popover_reset', enabled: true });
  command({ cmd: 'set_popover_reset_minutes', minutes: 1 });
  child.clickLocal('settings');
  visibility(false, { source: {} });
  visibility(false, { origin: 'https://foreign.example' });
  visibility(false, {}, { token: 'wrong' });
  visibility('false');
  child.advance(60000);
  visibility(true);
  assert.equal(child.element('groups').dataset.view, 'settings', 'invalid closes never start an interval');
  visibility(false);
  child.advance(60000);
  visibility(true, { source: {} });
  visibility(true, { origin: 'https://foreign.example' });
  visibility(true, {}, { token: 'wrong' });
  visibility('true');
  assert.equal(child.element('groups').dataset.view, 'settings', 'invalid opens neither reset nor consume the interval');
  visibility(true);
  assert.equal(child.element('groups').dataset.view, 'sessions');
});

test('native lamp enable actions restore the illustrative connection detail', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  child.clickLocal('lamp');
  for (const enabled of [false, true]) {
    child.clickCommand('set_led', enabled);
    // The USB row's Connection details carry the worker detail.
    assert.equal(child.element('lampdiagnostic-0').textContent, 'Simulated USB lamp. No hardware is connected by this website.');
    assert.doesNotMatch(child.element('lampdiagnostic-0').textContent, /Waiting for the service|Stopping lamp updates/);
  }
});

test('missing production assets report an actionable preview error', async () => {
  const demo = await preview({ fail: true });
  assert.equal(demo.frame.dataset.error, 'true');
  assert.equal(demo.events.at(-1).type, 'broglow:app-error');
  assert.equal(demo.frame.srcdoc, undefined);
});

test('aggregate messages accept real tray states without rebuilding sessions', async () => {
  const demo = await preview();
  demo.message({ type: 'broglow:app-aggregate', signal: 'arbitrary' });
  demo.message({ type: 'broglow:app-aggregate', signal: 'green', token: 'wrong' });
  assert.equal(demo.events.length, 0);
  demo.message({ type: 'broglow:app-aggregate', signal: 'green' });
  assert.equal(demo.events.length, 1);
  assert.equal(demo.events[0].detail.signal, 'green');
  demo.message({ type: 'broglow:app-aggregate', signal: 'none' });
  assert.equal(demo.events.at(-1).detail.signal, 'none');
  assert.equal(demo.posted.length, 0, 'aggregate updates do not repopulate the iframe');
});

test('only the authenticated app frame can activate its window', async () => {
  const demo = await preview();
  const activate = { type: 'broglow:app-activate' };
  demo.message(activate, { source: {} });
  demo.message(activate, { origin: 'https://foreign.example' });
  demo.message({ ...activate, token: 'wrong' });
  assert.equal(demo.events.length, 0);
  demo.message(activate);
  assert.equal(demo.events.at(-1).type, 'broglow:app-activate');
  assert.equal(demo.posted.length, 0, 'raising the panel does not reset sessions');
});

test('pointer and keyboard activation in the actual app notify the parent before session focus', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  for (const type of ['pointerdown', 'focusin']) {
    child.activate(type);
    assert.equal(child.sent.at(-1).type, 'broglow:app-activate');
    assert.equal(child.sent.at(-1).token, 'test-channel-token');
  }
  child.window.ipc.postMessage(JSON.stringify({ cmd: 'focus', id: 'codex' }));
  assert.equal(child.sent.at(-1).type, 'broglow:open-session', 'native session actions still raise the terminal last');
  assert.equal(child.element('aggline').textContent, '3 live sessions');
});

test('mixed-session settings survive scenario changes and dismissal clears the tray', async () => {
  const child = childPage((await preview()).frame.srcdoc);
  const greenIcon = child.element('hicon').src;
  const aggregate = () => child.sent.filter(message => message.type === 'broglow:app-aggregate').at(-1).signal;
  const command = message => child.window.ipc.postMessage(JSON.stringify(message));
  const send = signal => child.listeners.message({ source: child.parent, origin: 'http://localhost:8766',
    data: { token: 'test-channel-token', type: 'broglow:app-state', signal } });
  send('yellow');
  assert.equal(aggregate(), 'yellow');
  command({ cmd: 'set_yellow_mode', mode: 'active_wins' });
  assert.equal(aggregate(), 'green');
  assert.equal(child.element('hicon').src, greenIcon);
  send('yellow');
  assert.equal(aggregate(), 'green', 'new scenarios respect the chosen mixed-session policy');
  send('red');
  assert.equal(aggregate(), 'red', 'input always takes priority');
  command({ cmd: 'set_yellow_mode', mode: 'any_inactive' });
  send('yellow');
  command({ cmd: 'clear_paused' });
  assert.equal(aggregate(), 'green');
  assert.doesNotMatch(child.element('groups').innerHTML, /Test the checkout flow/);
  for (const id of ['claude', 'opencode']) command({ cmd: 'clear_session', id });
  assert.equal(aggregate(), 'none');
  assert.equal(child.element('aggline').textContent, 'No live sessions');
});
