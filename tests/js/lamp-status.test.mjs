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
  const session = (id, agent) => ({ id, name: 'Task ' + id, project: 'broglow', status: 'active',
    lastUpdated: new Date().toISOString(), agent });
  const sessions = [
    session('claude', { label: 'Claude', icon: 'M8 1.5v13', badge: '' }),
    session('future', { label: 'Mystery <b>', badge: 'my' }),
  ];
  const refresh = settings => context.window.__setState({ aggregate: 'green', sessions, settings, lamp: lamp() });
  const meta = id => {
    const html = element('groups').innerHTML;
    const row = html.slice(html.indexOf(`data-id="${id}"`));
    return row.match(/<span class="smeta">(.*?)<\/span><\/div>/)[1];
  };

  refresh({}); // Names show by default, including for Claude.
  assert.match(meta('claude'), /<svg class="agicon"[^>]*aria-hidden="true"><path d="M8 1.5v13"\/><\/svg>/);
  assert.equal(visibleLabel({ innerHTML: meta('claude') }), 'Claude · broglow');
  assert.match(meta('future'), /<span class="badge" aria-hidden="true">my<\/span>/);
  assert.equal(visibleLabel({ innerHTML: meta('future') }), 'myMystery &lt;b&gt; · broglow');
  assert.doesNotMatch(element('groups').innerHTML, /class="sname"[^>]*><span class="badge"/,
    'The title line carries no agent tag');

  refresh({ showAgentNames: false });
  assert.match(meta('claude'), /<svg class="agicon"[^>]*role="img" aria-label="Claude">/);
  assert.equal(visibleLabel({ innerHTML: meta('claude') }), 'broglow', 'No separator without a name');
  assert.match(meta('claude'), /title="Claude"/, 'Hover still names the agent');
  assert.equal(visibleLabel({ innerHTML: meta('future') }), 'mybroglow');
});

test('a host glyph follows the agent and names the host on hover', () => {
  const { context, element } = popoverPage();
  const agent = { label: 'Claude', icon: 'M8 1.5v13', badge: 'cc' };
  const sessions = [
    { id: 'app', name: 'Task', project: 'broglow', status: 'active', lastUpdated: new Date().toISOString(),
      agent, host: { kind: 'app', name: 'Claude app' } },
    { id: 'tty', name: 'Task', project: 'broglow', status: 'active', lastUpdated: new Date().toISOString(),
      agent, host: { kind: 'terminal', name: 'Ghost<ty>' } },
    { id: 'odd', name: 'Task', project: 'broglow', status: 'active', lastUpdated: new Date().toISOString(),
      agent, host: { kind: 'future', name: 'Somewhere' } },
  ];
  const refresh = settings => context.window.__setState({ aggregate: 'green', sessions, settings, lamp: lamp() });
  const meta = id => {
    const html = element('groups').innerHTML;
    return html.slice(html.indexOf(`data-id="${id}"`)).match(/<span class="smeta">(.*?)<\/span><\/div>/)[1];
  };
  const text = html => html.replace(/<svg[\s\S]*?<\/svg>/g, '[svg]').replace(/<[^>]*>/g, '');

  refresh({});
  assert.match(meta('app'), /<svg class="agicon aftername"[^>]*role="img" aria-label="Claude app"><title>Claude app<\/title>/);
  assert.equal(text(meta('app')), '[svg]Claude [svg] · broglow');
  assert.match(meta('tty'), /aria-label="Ghost&lt;ty&gt;"/, 'Host names are escaped');
  assert.equal(text(meta('odd')), '[svg]Claude · broglow', 'Unknown kinds draw no glyph');

  refresh({ showAgentNames: false });
  assert.match(meta('app'), /<svg class="agicon" viewBox="0 0 16 16" role="img" aria-label="Claude app">/);
  assert.equal(text(meta('app')), '[svg][svg]broglow');
});

test('Settings toggles agent names and waits for the saved preference', () => {
  const { context, element, click, listeners, commands } = popoverPage();
  const refresh = enabled => context.window.__setState({ aggregate: 'none', sessions: [],
    settings: { showAgentNames: enabled }, lamp: lamp() });
  refresh(undefined);
  click('settingsbtn');
  const toggle = element('showagentnames');
  assert.equal(toggle.checked, true, 'On unless turned off');
  assert.match(element('groups').innerHTML, /<label[^>]*for="showagentnames"/);
  toggle.focus();
  for (const enabled of [false, true]) {
    toggle.checked = enabled;
    listeners.change({ target: toggle });
    assert.deepEqual(commands('set_agent_names').at(-1), { cmd: 'set_agent_names', enabled });
    refresh(enabled);
    assert.equal(toggle.checked, enabled);
    assert.equal(context.document.activeElement, toggle, 'Keep keyboard focus through refresh');
  }
  assert.equal(commands('set_dashboard_button').length, 0);
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

test('usage shows plan percentages and tokens, never dollar estimates', () => {
  const { context, element } = popoverPage();
  const show = usage => {
    context.window.__setState({ aggregate: 'none', sessions: [], settings: { usageEnabled: true },
      lamp: lamp(), usage });
    return visibleLabel(element('usage'));
  };
  const plan = show({ todayTokens: 1240000, fiveHourPct: 62, fiveHourReset: '4:00 PM', weekPct: 31 });
  assert.match(plan, /62%/);
  assert.match(plan, /31%/);
  assert.match(plan, /Today ≈ 1\.2M tokens/);
  const tokensOnly = show({ todayTokens: 12400, fiveHourPct: null, weekPct: null });
  assert.match(tokensOnly, /≈ 12k tokens/);
  for (const text of [plan, tokensOnly]) assert.doesNotMatch(text, /\$|API/);
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
  const { element, update, click, listeners, commands, advance } = popoverPage();
  update(lamp({ bluetooth_supported: true }));
  click('lampbtn');
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
  click('lampsave-1');
  click('lampforget-1');
  click('lampadd');
  assert.equal(commands('rename_lamp').length, 1);
  assert.equal(commands('forget_lamp').length, 0);
  assert.equal(commands('add_lamp').length, 0);
  context.window.__configActionResult('Cannot save name');
  refresh({ 'home-id': 'Changed elsewhere' });
  assert.equal(element('lamperror').textContent, 'Cannot save name');
  assert.equal(input.value, '  Дом 🏡  ');
  assert.equal(element('lampsave-0').disabled, false);
  click('lampsave-0');
  refresh({ 'home-id': 'Дом 🏡' });
  context.window.__configActionResult(null);
  assert.equal(element('lamplabel-0').textContent, 'Дом 🏡');
  assert.equal(input.value, 'Дом 🏡');
  assert.equal(element('lamperror').hidden, true);
  assert.equal(list.writes, builds, 'Names and drafts do not rebuild disclosure controls');
});

test('Enter in a lamp name saves it like the Save button', () => {
  const { element, update, click, listeners, commands } = popoverPage();
  update(lamp({ state: 'searching', devices: [{ id: 'saved-lamp', state: 'searching' }], names: {} }));
  click('lampbtn');
  const input = element('lampname-0');
  input.value = ' Desk ';
  let prevented = false;
  listeners.keydown({ key: 'Enter', isComposing: true, target: input, preventDefault() {} });
  assert.equal(commands('rename_lamp').length, 0, 'IME composition Enter does not save');
  listeners.keydown({ key: 'Enter', target: input, preventDefault() { prevented = true; } });
  assert.deepEqual(commands('rename_lamp'), [{ cmd: 'rename_lamp', id: 'saved-lamp', name: 'Desk' }]);
  assert.ok(prevented);
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
    assert.doesNotMatch(list.innerHTML, /lampforgethelp/);
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
  const { element, update, click, commands } = popoverPage();
  update(lamp());
  click('lampbtn');
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
    for (const invalid of ['101', '-1', '0.5', '']) {
      slider.value = invalid;
      listeners.change({ target: slider });
    }
    assert.equal(commands('set_lamp_brightness').length, 0);
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
  for (const sliderId of ['brightness', 'lampbrightness-0']) {
    for (const end of ['blur', 'hidden', 'Escape', 'Back']) {
      const { context, element, listeners, windowListeners, click, update, commands, advance } = popoverPage();
      update(lamp());
      click('lampbtn');
      const slider = element(sliderId);
      listeners.pointerdown({ target: slider, pointerId: 1 });
      slider.value = '23';
      listeners.input({ target: slider });
      if (end === 'blur') windowListeners.blur();
      else if (end === 'hidden') { context.document.hidden = true; listeners.visibilitychange(); }
      else if (end === 'Escape') listeners.keydown({ key: 'Escape' });
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
