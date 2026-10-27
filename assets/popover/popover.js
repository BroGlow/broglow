
'use strict';
const { agentMark, tokens: fmtTokens } = window.BroGlowPresentation;
const ICONS = {
  red: "__ICON_RED__",
  yellow: "__ICON_YELLOW__",
  green: "__ICON_GREEN__",
  none: "__ICON_NONE__",
};
const GROUPS = [
  { status: 'needs_help', label: 'Needs help', dot: 'dred',    word: 'needs help' },
  { status: 'active',     label: 'Working',    dot: 'dgreen',  word: 'working' },
  { status: 'inactive',   label: 'Paused',     dot: 'dyellow', word: 'paused' },
];
const BY_STATUS = Object.fromEntries(GROUPS.map((g, rank) => [g.status, { ...g, rank }]));
// Folder glyph for project headings (our own geometry, placed in a `d` attribute).
const FOLDER_ICON = 'M1.5 4.5a1 1 0 0 1 1-1h3.2l1.6 1.6h6.2a1 1 0 0 1 1 1v6.4a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z';

const YELLOW_OPTIONS = [
  { value: 'any_inactive', title: 'Yellow when any session is idle' },
  { value: 'active_wins', title: 'Green while any session is working' },
];

let state = { aggregate: 'none', sessions: [], settings: { yellowMode: 'any_inactive' },
              lamp: { present: false, enabled: false, transport: 'usb', state: 'disabled' } };
let view = 'sessions';
let sessionPointer = null;

function releaseSessionPointer() {
  if (sessionPointer === null) return;
  sessionPointer = null;
  render();
}

function post(msg) { window.ipc.postMessage(JSON.stringify(msg)); }

function esc(s) {
  return String(s).replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[c]);
}

function renderUsage() {
  const el = document.getElementById('usage');
  const u = state.usage;
  const agents = (u?.agents || []).filter(a => a.todayTokens > 0 || a.limits?.length);
  const usageEnabled = pendingSettings.usageEnabled ?? state.settings?.usageEnabled;
  if (view !== 'sessions' || usageEnabled !== true || !agents.length) {
    el.hidden = true;
    el.innerHTML = '';
    return;
  }

  const html = `<div class="ulhead" role="heading" aria-level="2">Usage</div>` + agents.map(a => {
    let section = `<section class="uagent" aria-label="${esc(a.label)}">` +
      `<div class="uagenthead" role="heading" aria-level="3">${agentMark(a, true)}${esc(a.label)}</div>` +
      `<div class="usub">Today ≈ ${fmtTokens(a.todayTokens)} tokens</div>`;
    for (const limit of a.limits || []) {
      if (!Number.isFinite(limit.usedPercent)) continue;
      const pct = Math.max(0, Math.min(100, limit.usedPercent));
      section += `<div class="ulhead"><span>${esc(limit.label)}</span><span class="f1"></span>` +
        `<span class="uval">${Math.round(pct)}%</span>` +
        (limit.reset ? `<span class="usub">resets ${esc(limit.reset)}</span>` : '') +
        `</div><div class="ubar" role="progressbar" aria-label="${esc(a.label)} ${esc(limit.label)} used"` +
        ` aria-valuemin="0" aria-valuemax="100" aria-valuenow="${pct}">` +
        `<span class="ufill" style="width:${pct}%"></span></div>`;
    }
    return section + '</section>';
  }).join('') + `<div class="usub">All agents today ≈ ${fmtTokens(u.todayTokens)} tokens</div>`;
  el.innerHTML = html;
  el.hidden = false;
}

function lampState(status) {
  return ({ connected: 'Connected', connecting: 'Connecting', searching: 'Looking for lamp',
    error: 'Needs attention', unavailable: 'No recent response', disabled: 'Off' })[status] || 'No recent response';
}
let pendingForgetId = null, pendingRenameId = null, pendingIdentifyId = null, configActionError = '';
let savedRenameId = null;
const lampNameDrafts = new Map();
function lampActionPending() { return pendingForgetId !== null || pendingRenameId !== null || pendingIdentifyId !== null; }
// Keep submitted settings until the host reports the write; racing state
// pushes must not undo an edit before it has been saved.
let pendingSettings = {};
window.__configActionResult = error => {
  if (!error && pendingRenameId !== null) {
    lampNameDrafts.delete(pendingRenameId);
    savedRenameId = pendingRenameId;
    if (document.activeElement?.dataset.lampNameId === pendingRenameId) document.activeElement.blur();
  }
  pendingForgetId = null;
  pendingRenameId = null;
  pendingIdentifyId = null;
  if (error && 'popoverResetMinutes' in pendingSettings && view === 'settings') {
    document.getElementById('popoverresetminutes').value = String(state.settings?.popoverResetMinutes ?? 5);
  }
  pendingSettings = {};
  configActionError = error || '';
  render();
};
let lampSetupPending = false, lampSetupError = '', lampSetupMessage = '';
let firmwarePending = false, firmwareError = '', firmwareMessage = '', firmwareProgress = 0;
function addLampBusy() { return lampSetupPending || state.lamp?.setup_busy === true; }
function firmwareBusy() { return firmwarePending || state.lamp?.firmware?.busy === true; }
// Both own the USB cable; every lamp control waits for either.
function lampSetupBusy() { return addLampBusy() || firmwareBusy(); }
// A probed USB lamp must run BroGlow firmware of another version; while our own
// USB mirroring holds the cable there is no probe, and the write pauses it.
function usbUpdateAllowed(lamp) {
  const usb = lamp.usb_lamp;
  if (usb) return usb.broglow === true && usb.version !== lamp.firmware?.available;
  return lamp.usb_present === true && lamp.transport === 'usb' && lamp.enabled === true;
}
function canUpdateFirmware() {
  const lamp = state.lamp || {};
  return !!lamp.firmware?.available && usbUpdateAllowed(lamp) && !lampSetupBusy() && !lampActionPending();
}
window.__firmwareProgress = percent => {
  firmwareProgress = Math.max(0, Math.min(100, Number(percent) || 0));
  renderLamp();
};
window.__firmwareResult = (message, error) => {
  firmwarePending = false;
  if (state.lamp?.firmware) state.lamp.firmware.busy = false;
  firmwareError = error || '';
  firmwareMessage = error ? '' : (message || '');
  render();
};
function canIdentifyLamp(id) {
  const lamp = state.lamp || {};
  return !lampSetupBusy() && !lampActionPending() && lamp.transport === 'ble' &&
    lamp.enabled && lamp.state === 'connected' &&
    (lamp.devices || []).some(device => device.id === id && device.state === 'connected');
}
window.__lampSetupResult = error => {
  lampSetupPending = false;
  if (state.lamp) state.lamp.setup_busy = false;
  lampSetupError = error || '';
  lampSetupMessage = error ? '' : 'Lamp added.';
  render();
};

let previewSlider = null, previewPointer = null, previewTimer = null, previewKeepAlive = null;
let pendingPreview = null, lastPreviewAt = -Infinity;
function brightnessPercent(value) {
  const percent = Number(value);
  return String(value).trim() !== '' && Number.isInteger(percent) && percent >= 0 && percent <= 100 ? percent : null;
}
function isBrightnessSlider(element) {
  return element.id === 'brightness' || element.dataset.brightnessId !== undefined;
}
function canEditBrightness(slider) {
  const id = slider.dataset.brightnessId, lamp = state.lamp || {};
  return !lampSetupBusy() && !lampActionPending() && (id === undefined ||
    lamp.transport === 'ble' && (lamp.devices || []).some(device => device.id === id));
}
function canPreviewBrightness(id) {
  const lamp = state.lamp || {};
  return !lampSetupBusy() && !lampActionPending() && view === 'lamp' && lamp.transport === 'ble' && lamp.enabled && lamp.state === 'connected' &&
    (lamp.devices || []).some(device => device.state === 'connected' && (id === undefined || device.id === id));
}
function flushBrightnessPreview() {
  clearTimeout(previewTimer);
  previewTimer = null;
  const preview = pendingPreview;
  pendingPreview = null;
  if (preview !== null && canPreviewBrightness(preview.id)) {
    post({ cmd: 'preview_brightness', ...preview });
    lastPreviewAt = Date.now();
  }
}
function previewBrightness(slider, force = false) {
  const percent = brightnessPercent(slider.value), id = slider.dataset.brightnessId;
  if (percent === null) return;
  previewSlider = slider;
  if (!canPreviewBrightness(id)) {
    clearTimeout(previewTimer);
    previewTimer = pendingPreview = null;
    return;
  }
  pendingPreview = id === undefined ? { percent } : { percent, id };
  const wait = 80 - (Date.now() - lastPreviewAt);
  if (force || wait <= 0) flushBrightnessPreview();
  else if (previewTimer === null) previewTimer = setTimeout(flushBrightnessPreview, wait);
}
function stopBrightnessPreview() {
  if (previewSlider && (previewPointer !== null || pendingPreview !== null)) {
    previewBrightness(previewSlider, true);
  }
  clearTimeout(previewTimer);
  clearInterval(previewKeepAlive);
  previewSlider = previewPointer = previewTimer = previewKeepAlive = pendingPreview = null;
}
function renderBrightness(slider, outputId, hintId, percent) {
  slider.dataset.brightnessOutput = outputId;
  slider.disabled = !canEditBrightness(slider);
  // Keep the exact native control being edited intact across host heartbeats.
  if (!(previewPointer !== null && previewSlider === slider) &&
      document.activeElement !== slider && !slider.matches(':active')) slider.value = String(percent);
  document.getElementById(outputId).textContent = slider.value + '%';
  const targeted = slider.dataset.brightnessId !== undefined;
  const hint = document.getElementById(hintId);
  hint.textContent = lampSetupBusy() || lampActionPending() ? 'Lamp action in progress.' :
    canPreviewBrightness(slider.dataset.brightnessId) ? '' :
    state.lamp?.enabled ? (targeted ? 'Connect this lamp to preview.' : 'Connect a lamp to preview.') : 'Enable a lamp to preview.';
  if (!targeted && state.lamp?.transport === 'ble') {
    hint.textContent += (hint.textContent ? ' ' : '') + 'Replaces individual brightness settings.';
  }
  hint.hidden = !hint.textContent;
}

/* Connection comes from worker writes, never from a nearby advertisement or
   enabled setting. Diagnostics and controls live in the Lamps view. */
function renderLamp() {
  const l = state.lamp || {};
  const setupBusy = lampSetupBusy();
  if (setupBusy || lampActionPending()) stopBrightnessPreview();
  const el = document.getElementById('lampbtn');
  const wireless = l.transport === 'ble';
  const status = l.enabled ? (l.state || 'unavailable') : 'disabled';
  const devices = l.devices || [];
  const connectedCount = l.enabled && status === 'connected' ? devices.filter(device => device.state === 'connected').length : 0;
  const connected = status === 'connected' && (!wireless || connectedCount > 0);
  const plural = wireless && l.selected_count > 1;
  let dot = null, label = plural ? 'Lamps' : 'Lamp';
  if (l.enabled) {
    dot = connected ? 'dgreen' : (status === 'error' || status === 'unavailable' ? 'dred' : 'dyellow');
    if (plural) label += ` ${connectedCount}/${l.selected_count}`;
  } else {
    label += ' off';
  }
  el.className = 'pbtn pbtnghost lampbtn' + (connected ? ' lon' : '');
  el.title = `${label} — ${lampState(connected ? 'connected' : status === 'connected' ? 'unavailable' : status)}`;
  el.setAttribute('aria-label', el.title);
  el.setAttribute('aria-expanded', String(view === 'lamp'));
  el.innerHTML = (dot ? `<span class="dot ${dot}" aria-hidden="true"></span>` : '') + esc(label);
  if (view === 'lamp') {
    document.getElementById('lampsetup').hidden = l.bluetooth_supported !== true;
    const add = document.getElementById('lampadd');
    add.dataset.cmd = 'add_lamp';
    add.disabled = setupBusy;
    add.setAttribute('aria-disabled', String(setupBusy || lampActionPending()));
    add.textContent = addLampBusy() ? 'Adding…' : 'Add lamp';
    const setupResult = document.getElementById('lampsetupresult');
    setupResult.textContent = addLampBusy() ? 'Setting up USB lamp…' : lampSetupError || lampSetupMessage;
    setupResult.className = !addLampBusy() && lampSetupError ? 'error' : '';
    setupResult.setAttribute('role', !addLampBusy() && lampSetupError ? 'alert' : 'status');
    setupResult.hidden = !setupResult.textContent;
    document.getElementById('lamptransport').textContent = wireless ? 'Bluetooth' : 'USB';
    document.getElementById('lampsummary').textContent = wireless && !l.selected_count ? 'No lamps added' :
      !l.enabled ? 'Updates paused' : plural ? `${connectedCount} of ${l.selected_count} connected` :
      lampState(connected ? 'connected' : status === 'connected' ? 'unavailable' : status);
    const rows = wireless ? devices : [{ id: 'usb', state: status, detail: l.detail }];
    const list = document.getElementById('lamplist');
    const ids = JSON.stringify([l.transport, ...rows.map(device => device.id)]);
    // Preserve native disclosure state, focus and text selection across heartbeats.
    if (list.dataset.ids !== ids) {
      if (previewSlider?.dataset.brightnessId !== undefined) stopBrightnessPreview();
      list.innerHTML = rows.map((device, i) =>
        `<details class="lampitem"><summary class="lampheading">` +
        `<span class="lamptitle" id="lamplabel-${i}">${wireless ? 'Lamp ' + (i + 1) : 'USB lamp'}</span>` +
        `<span class="lampstate" id="lampstate-${i}"></span>` +
        `<span class="lampchevron" aria-hidden="true">›</span></summary>` +
        (wireless ? `<div class="lampnameedit"><label for="lampname-${i}">Name</label><div class="lampnamefields">` +
          `<input id="lampname-${i}" type="text" autocomplete="off" placeholder="Automatic name">` +
          `<button class="pbtn" type="button" id="lampsave-${i}">Save</button></div>` +
          `<p class="lampnamesaved" id="lampnamesaved-${i}" role="status" hidden></p></div>` +
          `<div class="lampidentify"><button class="pbtn" type="button" id="lampidentify-${i}">Identify</button></div>` +
          `<div class="brightness"><label for="lampbrightness-${i}">Light brightness<output id="lampbrightnessvalue-${i}" for="lampbrightness-${i}"></output></label>` +
          `<input id="lampbrightness-${i}" type="range" min="0" max="100" step="1" value="100" aria-describedby="lampbrightnesshint-${i}">` +
          `<p id="lampbrightnesshint-${i}"></p></div>` : '') +
        `<details class="lamptechnical"><summary>Connection details</summary>` +
        `<dl><dt>${wireless ? 'Device ID' : 'Connection'}</dt><dd class="lampid" id="lampid-${i}"></dd>` +
        (wireless ? `<dt>Firmware</dt><dd id="lampfirmware-${i}"></dd>` : '') +
        `<dt>Diagnostic</dt><dd id="lampdiagnostic-${i}"></dd></dl></details>` +
        (wireless ? `<div class="lampforget"><button class="pbtn pbtnghost pbtndanger" type="button" id="lampforget-${i}" ` +
          `aria-label="Forget lamp ${i + 1}">Forget lamp</button></div>` : '') +
        `</details>`
      ).join('');
      list.dataset.ids = ids;
    }
    rows.forEach((device, i) => {
      const savedName = wireless ? (l.names?.[device.id] || '') : '';
      const label = document.getElementById('lamplabel-' + i);
      const name = savedName || (wireless ? 'Lamp ' + (i + 1) : 'USB lamp');
      if (label.textContent !== name) label.textContent = name;
      const deviceState = !l.enabled ? 'disabled' :
        device.state === 'connected' && status !== 'connected' ? status : (device.state || status);
      const cell = document.getElementById('lampstate-' + i);
      cell.textContent = lampState(deviceState);
      cell.className = 'lampstate ' + (['connected', 'disabled', 'error', 'unavailable'].includes(deviceState) ? deviceState : '');
      const id = document.getElementById('lampid-' + i);
      const diagnostic = document.getElementById('lampdiagnostic-' + i);
      const identifier = wireless ? device.id : 'USB cable';
      if (id.textContent !== identifier) id.textContent = identifier;
      if (diagnostic.textContent !== (device.detail || '')) diagnostic.textContent = device.detail || '';
      if (wireless) {
        const firmwareCell = document.getElementById('lampfirmware-' + i);
        const firmwareText = device.firmware || 'Unknown (connect by USB to check)';
        if (firmwareCell.textContent !== firmwareText) firmwareCell.textContent = firmwareText;
        const brightness = document.getElementById('lampbrightness-' + i);
        brightness.dataset.brightnessId = device.id;
        renderBrightness(brightness, 'lampbrightnessvalue-' + i, 'lampbrightnesshint-' + i,
          l.brightness_by_id?.[device.id] ?? l.brightness_percent ?? 100);
        const identify = document.getElementById('lampidentify-' + i);
        identify.dataset.cmd = 'identify_lamp';
        identify.dataset.id = device.id;
        identify.disabled = !canIdentifyLamp(device.id);
        identify.textContent = pendingIdentifyId === device.id ? 'Sending…' : 'Identify';
        const input = document.getElementById('lampname-' + i);
        input.dataset.lampNameId = device.id;
        input.disabled = setupBusy || lampActionPending();
        const draft = lampNameDrafts.has(device.id) ? lampNameDrafts.get(device.id) : savedName;
        if (document.activeElement !== input && input.value !== draft) input.value = draft;
        const save = document.getElementById('lampsave-' + i);
        save.dataset.cmd = 'rename_lamp';
        save.dataset.id = device.id;
        save.dataset.input = input.id;
        save.disabled = setupBusy || lampActionPending();
        save.textContent = pendingRenameId === device.id ? 'Saving…' : 'Save';
        const saved = document.getElementById('lampnamesaved-' + i);
        saved.textContent = savedRenameId === device.id ? 'Name saved.' : '';
        saved.hidden = savedRenameId !== device.id;
        const forget = document.getElementById('lampforget-' + i);
        forget.dataset.cmd = 'forget_lamp';
        forget.dataset.id = device.id;
        forget.disabled = setupBusy || lampActionPending();
        forget.textContent = pendingForgetId === device.id ? 'Forgetting…' : 'Forget lamp';
      }
    });
    const error = document.getElementById('lamperror');
    error.textContent = configActionError;
    error.hidden = !configActionError;
    const detail = document.getElementById('lampdetail');
    detail.textContent = wireless && !l.selected_count ?
      (l.bluetooth_supported ? 'Connect one lamp by USB, then use Add lamp above.' : 'Bluetooth lamp setup requires macOS.') : '';
    detail.hidden = !detail.textContent;
    const control = document.getElementById('lampcontrol');
    control.dataset.cmd = 'set_led';
    control.disabled = setupBusy || (!l.enabled && !l.present && !wireless);
    control.dataset.enabled = String(!l.enabled);
    control.textContent = (l.enabled ? 'Disable' : 'Enable') + (plural ? ' lamps' : ' lamp');
    document.getElementById('brightnesscontrol').hidden = !wireless;
    renderBrightness(document.getElementById('brightness'), 'brightnessvalue', 'brightnesshint', l.brightness_percent ?? 100);
    renderFirmware(l, wireless ? devices : []);
  }
}

// The firmware section exists only when this app carries release images.
function renderFirmware(l, devices) {
  const firmware = l.firmware || {};
  const section = document.getElementById('lampfirmware');
  section.hidden = !firmware.available;
  if (!firmware.available) return;
  const busy = firmwareBusy();
  if ((firmware.progress || 0) > firmwareProgress) firmwareProgress = firmware.progress;
  const versions = devices.map(device => device.firmware);
  const outdated = [...new Set(versions.filter(version => version && version !== firmware.available))];
  const unknown = versions.filter(version => !version).length;
  const usb = l.usb_lamp;
  const usbName = usb?.id ? (l.names?.[usb.id] ||
    (devices.findIndex(device => device.id === usb.id) >= 0 ? `Lamp ${devices.findIndex(device => device.id === usb.id) + 1}` : 'Lamp')) :
    'The lamp';
  const upToDate = !busy && usb?.broglow === true && usb.version === firmware.available;
  let hint;
  if (busy) hint = `Writing firmware ${firmware.available}… ${firmwareProgress}%. Keep the lamp connected.`;
  else if (usb && !usb.broglow) hint = `The USB device gives no BroGlow reply. For a new or damaged board run \`broglow flash --force\` from a terminal.`;
  else if (upToDate) hint = `${usbName} on USB runs firmware ${firmware.available}, the latest.`;
  else if (usb) hint = `${usbName} on USB runs firmware ${usb.version || `older than ${firmware.available}`}. Updating pauses lamp control; Bluetooth pairing is kept.`;
  else if (l.usb_present && l.transport === 'usb' && l.enabled) hint = `Firmware ${firmware.available} is ready for the lamp on USB. Updating pauses lamp control; Bluetooth pairing is kept.`;
  else if (l.usb_present) hint = 'Checking the lamp on USB…';
  else if (outdated.length) hint = `${versions.filter(version => outdated.includes(version)).length === 1 ? 'A lamp runs' : 'Lamps run'} firmware ${outdated.join(', ')}; ${firmware.available} is available. Connect the lamp by USB to update it.`;
  else if (versions.length && !unknown) hint = `Firmware ${firmware.available} is up to date.`;
  else hint = `Connect a lamp by USB to check or update its firmware (${firmware.available} available).`;
  document.getElementById('lampfirmwarehint').textContent = hint;
  const button = document.getElementById('lampflash');
  button.dataset.cmd = 'update_firmware';
  button.hidden = upToDate;
  button.disabled = !canUpdateFirmware();
  button.setAttribute('aria-disabled', String(!canUpdateFirmware()));
  button.textContent = busy ? 'Updating…' : usb?.id ? `Update ${usbName} to ${firmware.available}` : `Update firmware to ${firmware.available}`;
  const result = document.getElementById('lampfirmwareresult');
  result.textContent = busy ? '' : firmwareError || firmwareMessage;
  result.className = firmwareError && !busy ? 'error' : '';
  result.setAttribute('role', firmwareError && !busy ? 'alert' : 'status');
  result.hidden = !result.textContent;
}

// Where a session runs, drawn after the agent: window, >_ prompt, </> code.
const HOST_ICONS = {
  app: 'M2.5 3h11a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM1.5 6h13',
  terminal: 'M2.5 4.5 6 8l-3.5 3.5M8 12h5.5',
  ide: 'M5 4.5 1.5 8 5 11.5M11 4.5 14.5 8 11 11.5M9.5 3l-3 10',
};

// "<icon> Codex <host> · " before the project; with names off, just
// "<icon> <host>". The agent icon is path data from the host's agent registry,
// placed only in an attribute; hover names the agent and the host.
function agentMeta(agent, showName, host) {
  if (!agent || !agent.label) return '';
  const label = esc(agent.label);
  const mark = agentMark(agent, showName);
  const hostPath = host && HOST_ICONS[host.kind];
  const hostName = hostPath ? esc(host.name) : '';
  const hostMark = hostPath
    ? `<svg class="agicon${showName ? ' aftername' : ''}" viewBox="0 0 16 16" role="img" aria-label="${hostName}">` +
      `<title>${hostName}</title><path d="${hostPath}"/></svg>`
    : '';
  return showName
    ? `<span class="agent" title="${label}">${mark}${label}</span>${hostMark ? ' ' + hostMark : ''} · `
    : `<span class="agent" title="${label}">${mark}</span>${hostMark}`;
}

function relTime(iso) {
  const t = Date.parse(iso);
  if (isNaN(t)) return '';
  const s = Math.max(0, (Date.now() - t) / 1000);
  if (s < 60) return 'now';
  if (s < 3600) return Math.floor(s / 60) + 'm';
  if (s < 86400) return Math.floor(s / 3600) + 'h';
  return Math.floor(s / 86400) + 'd';
}

function render() {
  const focusedDismiss = ['clear_session', 'clear_paused'].includes(document.activeElement?.dataset.cmd) ? document.activeElement.id : null;
  document.getElementById('dashboardbtn').hidden = state.settings?.showDashboardButton !== true;
  const sessions = state.sessions;
  const counts = { needs_help: 0, active: 0, inactive: 0 };
  for (const s of sessions) if (s.status in counts) counts[s.status]++;

  document.getElementById('hicon').src = ICONS[state.aggregate] || ICONS.none;

  const live = sessions.length;
  let agg;
  if (live === 0) agg = 'No live sessions';
  else agg = live + ' live session' + (live === 1 ? '' : 's')
       + (counts.needs_help ? ' · ' + counts.needs_help + ' needs help' : '');
  document.getElementById('aggline').textContent = agg;

  const chipDefs = [
    ['dgreen', counts.active, 'working'], ['dyellow', counts.inactive, 'paused'], ['dred', counts.needs_help, 'needs help'],
  ];
  // The colour is the only visual legend; name the count for hover and
  // assistive tech, since the grouped-by-project list has no status headings.
  document.getElementById('chips').innerHTML = chipDefs
    .filter(([, n]) => n > 0)
    .map(([dot, n, word]) => `<span class="chip" role="img" title="${n} ${word}" aria-label="${n} ${word}"><span class="dot ${dot}"></span>${n}</span>`)
    .join('');

  const settingsBtn = document.getElementById('settingsbtn');
  settingsBtn.textContent = view === 'sessions' ? 'Settings' : 'Back';
  settingsBtn.dataset.local = view === 'sessions' ? 'settings' : 'sessions';

  const groupsEl = document.getElementById('groups');
  if (view === 'lamp') {
    // Reuse the controls: replacing this subtree during a drag loses native focus.
    if (groupsEl.dataset.view !== 'lamp') {
      groupsEl.innerHTML = `<div class="grplabel">Lamps</div>` +
        `<div class="lampsetup" id="lampsetup"><button class="pbtn" type="button" id="lampadd" aria-describedby="lampsetuphint">Add lamp</button>` +
        `<p id="lampsetuphint">Connect one lamp by USB to add it automatically. Approve any macOS pairing request. Use Add lamp to retry.</p>` +
        `<p id="lampsetupresult" role="status" hidden></p></div>` +
        `<div class="lamptop"><span id="lampsummary" role="status"></span><span id="lamptransport"></span></div>` +
        `<div class="lamplist" id="lamplist"></div>` +
        `<p class="lamperror" id="lamperror" role="alert" hidden></p>` +
        `<div class="brightness" id="brightnesscontrol"><label for="brightness">All lamps` +
        `<output id="brightnessvalue" for="brightness"></output></label>` +
        `<input id="brightness" type="range" min="0" max="100" step="1" value="100" aria-describedby="brightnesshint">` +
        `<p id="brightnesshint"></p></div>` +
        `<div class="lampactions"><button class="pbtn" id="lampcontrol"></button></div>` +
        `<div class="lampsetup" id="lampfirmware" hidden><div class="grplabel">Firmware</div>` +
        `<p id="lampfirmwarehint"></p>` +
        `<button class="pbtn" type="button" id="lampflash" aria-describedby="lampfirmwarehint">Update firmware</button>` +
        `<p id="lampfirmwareresult" role="status" hidden></p></div>` +
        `<p class="lampdetail" id="lampdetail" hidden></p>`;
    }
  } else if (view === 'settings') {
    // Build the controls once; later state pushes only sync their values.
    // Every hook write and lamp heartbeat pushes state, and replacing this
    // subtree each time destroyed the control under the pointer — clicks
    // landed on nothing while sessions were busy — and dropped focus.
    const mode = (state.settings && state.settings.yellowMode) || 'any_inactive';
    if (groupsEl.dataset.view !== 'settings') groupsEl.innerHTML =
      `<div class="grplabel">Appearance</div>` +
      `<label class="optrow" for="showdashboard">` +
      `<input type="checkbox" id="showdashboard" aria-describedby="dashboardhint">` +
      `<span class="sinfo"><span class="sname">Show Dashboard button</span>` +
      `<span class="smeta setdesc" id="dashboardhint">Terminal session monitor.</span></span></label>` +
      `<label class="optrow" for="showagentnames">` +
      `<input type="checkbox" id="showagentnames" aria-describedby="agentnameshint">` +
      `<span class="sinfo"><span class="sname">Show agent names</span>` +
      `<span class="smeta setdesc" id="agentnameshint">Name the coding agent beside its icon.</span></span></label>` +
      `<label class="optrow" for="groupbyproject">` +
      `<input type="checkbox" id="groupbyproject" aria-describedby="groupbyprojecthint">` +
      `<span class="sinfo"><span class="sname">Group by project</span>` +
      `<span class="smeta setdesc" id="groupbyprojecthint">List sessions by working directory, not by status.</span></span></label>` +
      `<div class="optrow resetsetting" role="group" aria-label="Return to chats after closing">` +
      `<div class="resetline"><label class="resetlabel" for="popoverreset">` +
      `<input type="checkbox" id="popoverreset" aria-describedby="popoverresethint">` +
      `<span class="sname">Return to chats after</span></label>` +
      `<input type="number" id="popoverresetminutes" min="1" max="1440" step="1" aria-label="Minutes closed before returning to chats" aria-describedby="popoverresethint">` +
      `<span class="smeta" aria-hidden="true">min</span></div>` +
      `<span class="smeta setdesc" id="popoverresethint">While the popover is closed.</span></div>` +
      `<p class="lamperror" id="settingserror" role="alert" hidden></p>` +
      `<div class="grplabel" id="mixedlabel">Mixed sessions</div>` +
      `<div role="radiogroup" aria-labelledby="mixedlabel">` +
      YELLOW_OPTIONS.map(o =>
        `<label class="optrow" for="yellow-${o.value}">` +
        `<input type="radio" name="yellowmode" id="yellow-${o.value}" value="${o.value}">` +
        `<span class="sinfo"><span class="sname">${o.title}</span></span></label>`
      ).join('') + `</div>` +
      `<div id="usblampsettings"><div class="grplabel">Lamps</div>` +
      `<label class="optrow" for="autoaddforgottenusb">` +
      `<input type="checkbox" id="autoaddforgottenusb" aria-describedby="autoaddforgottenusbhint">` +
      `<span class="sinfo"><span class="sname">Automatically add forgotten lamps over USB</span>` +
      `<span class="smeta setdesc" id="autoaddforgottenusbhint">Unplug a forgotten lamp for at least five seconds, then reconnect it to add it again. Turn this off if you connect lamps only for power.</span></span></label></div>` +
      `<div class="grplabel">Usage tracking</div>` +
      // Opt-in: turning it on lets BroGlow read local agent transcripts and
      // Claude's usage endpoint — hence the off default.
      `<label class="optrow" for="trackusage">` +
      `<input type="checkbox" id="trackusage" aria-describedby="usagehint">` +
      `<span class="sinfo"><span class="sname">Track usage</span>` +
      `<span class="smeta setdesc" id="usagehint">Separate Claude and Codex limits, plus local tokens for Claude, Codex and opencode (including API use). No token pricing.</span></span></label>` +
      `<label class="optrow" for="showusageinmenubar">` +
      `<input type="checkbox" id="showusageinmenubar" aria-describedby="showusageinmenubarhint">` +
      `<span class="sinfo"><span class="sname">Show usage in menu bar</span>` +
      `<span class="smeta setdesc" id="showusageinmenubarhint">Limits and tokens beside the status icon. Hiding them keeps tracking and popover totals on.</span></span></label>`;
    const setting = (key, saved) => key in pendingSettings ? pendingSettings[key] : saved;
    document.getElementById('showdashboard').checked = setting('showDashboardButton', state.settings?.showDashboardButton === true);
    document.getElementById('showagentnames').checked = setting('showAgentNames', state.settings?.showAgentNames !== false);
    document.getElementById('groupbyproject').checked = setting('groupByProject', state.settings?.groupByProject === true);
    const resetEnabled = setting('popoverResetEnabled', state.settings?.popoverResetEnabled !== false);
    document.getElementById('popoverreset').checked = resetEnabled;
    const resetMinutes = document.getElementById('popoverresetminutes');
    resetMinutes.disabled = !resetEnabled;
    if (document.activeElement !== resetMinutes) {
      resetMinutes.value = String(setting('popoverResetMinutes', state.settings?.popoverResetMinutes ?? 5));
    }
    document.getElementById('usblampsettings').hidden = !state.lamp?.bluetooth_supported;
    document.getElementById('autoaddforgottenusb').checked = setting('autoAddForgottenUsb', state.settings?.autoAddForgottenUsb !== false);
    document.getElementById(`yellow-${mode}`).checked = true;
    const usageEnabled = setting('usageEnabled', state.settings?.usageEnabled === true);
    document.getElementById('trackusage').checked = usageEnabled;
    const menuBarUsage = document.getElementById('showusageinmenubar');
    menuBarUsage.checked = setting('showUsageInMenuBar', state.settings?.showUsageInMenuBar !== false);
    menuBarUsage.disabled = !usageEnabled;
    const error = document.getElementById('settingserror');
    error.textContent = configActionError;
    error.hidden = !configActionError;
  } else if (sessionPointer !== null && groupsEl.dataset.view === 'sessions') {
    // Keep the pressed row attached until the browser delivers its click.
  } else if (live === 0) {
    groupsEl.innerHTML =
      `<div class="emptybox"><img src="${ICONS.none}" alt="">` +
      `<span>No live sessions</span></div>`;
  } else {
    const showAgentNames = state.settings?.showAgentNames !== false;
    const byRecency = (a, b) => Date.parse(b.lastUpdated) - Date.parse(a.lastUpdated);
    const clearPaused = `<button class="pbtn pbtnghost" type="button" id="clearpaused" data-cmd="clear_paused"` +
      ` title="Dismiss all paused sessions until new activity">Clear paused</button>`;
    // A session row; the meta line names the agent and host, then `detail`:
    // the project under status headings, the status word under project ones.
    const row = (s, detail) => {
      const help = s.status === 'needs_help';
      const capacity = help && s.capacityBlocked === true;
      const meta = agentMeta(s.agent, showAgentNames, s.host) + esc(detail);
      const action = capacity ? 'Open Codex' : 'Focus';
      const tail = help
        ? `<button class="focusbtn" type="button" data-cmd="focus" data-id="${esc(s.id)}"` +
          ` aria-label="${esc(action + ': ' + s.name)}"` +
          (capacity ? ` aria-describedby="capacity-${esc(s.id)}"` : '') + `>${action}</button>`
        : `<span class="time">${relTime(s.lastUpdated)}</span>`;
      return `<div class="srow${help ? ' helprow' : ''}" data-cmd="focus"` +
        ` data-id="${esc(s.id)}" title="${capacity ? 'Open Codex' : 'Open session'}">` +
        `<span class="dot ${BY_STATUS[s.status].dot}"></span>` +
        `<div class="sinfo"><span class="sname" title="${esc(s.name)}">${esc(s.name)}</span>` +
        `<span class="smeta">${meta}</span>` +
        (capacity ? `<span class="sreason">Model at capacity</span>` +
          `<span class="smeta capacityhint" id="capacity-${esc(s.id)}">Choose another model or retry in Codex.</span>` : '') +
        `</div>${tail}` +
        `<button class="dismissbtn" type="button" id="dismiss-${esc(s.id)}" data-cmd="clear_session" data-id="${esc(s.id)}"` +
        ` aria-label="Dismiss ${esc(s.name)} from BroGlow" title="Dismiss until new activity">×</button></div>`;
    };
    let html = '';
    if (state.settings?.groupByProject === true) {
      // One heading per working directory: the full path is the key (two
      // folders can share a name) and `project` is the host's label, already
      // extended with parent folders where names collide. Most urgent project
      // first, then the most recently active; sessions without a known
      // directory come last.
      const projects = new Map();
      for (const s of sessions) {
        if (!(s.status in BY_STATUS)) continue;
        const key = s.projectPath || '';
        if (!projects.has(key)) projects.set(key, { key, label: s.project, rows: [] });
        projects.get(key).rows.push(s);
      }
      const byUrgency = (a, b) => BY_STATUS[a.status].rank - BY_STATUS[b.status].rank || byRecency(a, b);
      const groups = [...projects.values()];
      for (const p of groups) p.rows.sort(byUrgency);
      groups.sort((a, b) => (a.key === '') - (b.key === '') || byUrgency(a.rows[0], b.rows[0]) || (a.key < b.key ? -1 : 1));
      if (groups.length) html += `<div class="grphead"><div class="grplabel">By project</div>` + (counts.inactive ? clearPaused : '') + `</div>`;
      for (const p of groups) {
        // The path is hover text for the mouse and part of the accessible
        // name for everyone else.
        html += `<div class="projhead" role="heading" aria-level="3"` +
          (p.key ? ` title="${esc(p.key)}" aria-label="${esc(p.label)}, ${esc(p.key)}"` : '') + `>` +
          (p.key ? `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="${FOLDER_ICON}"/></svg>` : '') +
          `<span>${esc(p.label)}</span></div>`;
        for (const s of p.rows) html += row(s, BY_STATUS[s.status].word);
      }
    } else {
      for (const g of GROUPS) {
        const rows = sessions.filter(s => s.status === g.status);
        if (!rows.length) continue;
        rows.sort(byRecency);
        html += `<div class="grphead"><div class="grplabel">${g.label}</div>` +
          (g.status === 'inactive' ? clearPaused : '') + `</div>`;
        for (const s of rows) html += row(s, s.project);
      }
    }
    groupsEl.innerHTML = html;
  }
  groupsEl.dataset.view = view;
  const sessionError = document.getElementById('sessionerror');
  sessionError.textContent = configActionError;
  sessionError.hidden = view !== 'sessions' || !configActionError;
  if (view === 'sessions' && focusedDismiss) (document.getElementById(focusedDismiss) || settingsBtn).focus();

  renderUsage();
  renderLamp();

  // Post synchronously: offsetHeight forces layout, and requestAnimationFrame
  // never fires while the window is hidden (no display link), which would
  // deadlock the deferred first show.
  // dpr rides along because on Windows the host sizes the window in physical
  // pixels from it — the only scale guaranteed to match the rendered surface.
  post({ cmd: 'resize', height: Math.ceil(document.getElementById('card').offsetHeight),
         dpr: window.devicePixelRatio });
}

window.__setState = (payload, resetNavigation = false) => {
  state = payload;
  if (resetNavigation) {
    stopBrightnessPreview();
    sessionPointer = null;
    view = 'sessions';
  }
  render();
};

document.addEventListener('toggle', ev => {
  if (ev.target.matches('details')) {
    post({ cmd: 'resize', height: Math.ceil(document.getElementById('card').offsetHeight),
           dpr: window.devicePixelRatio });
  }
}, true);

document.addEventListener('input', ev => {
  if (ev.target.dataset.lampNameId !== undefined) {
    if (!lampSetupBusy() && !lampActionPending()) {
      lampNameDrafts.set(ev.target.dataset.lampNameId, ev.target.value);
      if (savedRenameId === ev.target.dataset.lampNameId) {
        savedRenameId = null;
        render();
      }
    }
    return;
  }
  if (isBrightnessSlider(ev.target)) {
    if (!canEditBrightness(ev.target)) return;
    const percent = brightnessPercent(ev.target.value);
    if (percent === null) return;
    document.getElementById(ev.target.dataset.brightnessOutput).textContent = percent + '%';
    previewBrightness(ev.target);
  }
});
document.addEventListener('pointerdown', ev => {
  if (view === 'sessions' && ev.target.closest?.('[data-cmd]') &&
      (ev.button === undefined || ev.button === 0)) sessionPointer = ev.pointerId;
  if (!isBrightnessSlider(ev.target) || !canEditBrightness(ev.target) || (ev.button !== undefined && ev.button !== 0)) return;
  stopBrightnessPreview();
  previewPointer = ev.pointerId;
  previewBrightness(ev.target, true);
  previewKeepAlive = setInterval(() => { if (previewSlider) previewBrightness(previewSlider, true); }, 500);
});
for (const event of ['pointerup', 'pointercancel']) {
  document.addEventListener(event, ev => {
    if (previewPointer !== null && ev.pointerId === previewPointer) stopBrightnessPreview();
    if (sessionPointer !== null && ev.pointerId === sessionPointer) {
      // pointerup runs before click; apply the latest state after both events.
      setTimeout(() => { if (sessionPointer === ev.pointerId) releaseSessionPointer(); }, 0);
    }
  });
}
window.addEventListener('blur', () => { stopBrightnessPreview(); releaseSessionPointer(); });
document.addEventListener('visibilitychange', () => {
  if (document.hidden) { stopBrightnessPreview(); releaseSessionPointer(); }
});
document.addEventListener('change', ev => {
  // The checkbox keeps its clicked value (pendingSettings) until the host
  // confirms the write; what the value drives (the footer button, the row
  // meta, the list order) follows the saved state the host pushes back.
  const toggles = {
    showdashboard: ['showDashboardButton', 'set_dashboard_button'],
    showagentnames: ['showAgentNames', 'set_agent_names'],
    groupbyproject: ['groupByProject', 'set_group_by_project'],
    autoaddforgottenusb: ['autoAddForgottenUsb', 'set_auto_add_forgotten_usb'],
    popoverreset: ['popoverResetEnabled', 'set_popover_reset'],
    trackusage: ['usageEnabled', 'set_usage'],
    showusageinmenubar: ['showUsageInMenuBar', 'set_usage_in_menu_bar'],
  };
  if (ev.target.id in toggles) {
    if (ev.target.disabled) return;
    const [key, cmd] = toggles[ev.target.id];
    configActionError = '';
    document.getElementById('settingserror').hidden = true;
    pendingSettings[key] = ev.target.checked;
    if (ev.target.id === 'popoverreset' || ev.target.id === 'trackusage') render();
    post({ cmd, enabled: ev.target.checked });
    return;
  }
  if (ev.target.id === 'popoverresetminutes') {
    if (ev.target.disabled) return;
    const minutes = Number(ev.target.value);
    if (!Number.isInteger(minutes) || minutes < 1 || minutes > 1440) {
      configActionError = 'Use a whole number of minutes from 1 to 1440.';
      render();
      return;
    }
    configActionError = '';
    document.getElementById('settingserror').hidden = true;
    pendingSettings.popoverResetMinutes = minutes;
    post({ cmd: 'set_popover_reset_minutes', minutes });
    return;
  }
  // Update optimistically; the host confirms with a fresh __setState after
  // writing the config.
  if (ev.target.name === 'yellowmode') {
    state.settings = state.settings || {};
    state.settings.yellowMode = ev.target.value;
    post({ cmd: 'set_yellow_mode', mode: ev.target.value });
    return;
  }
  if (!isBrightnessSlider(ev.target) || !canEditBrightness(ev.target)) return;
  const percent = brightnessPercent(ev.target.value);
  if (percent === null) return;
  previewBrightness(ev.target, true);
  configActionError = '';
  document.getElementById('lamperror').hidden = true;
  const id = ev.target.dataset.brightnessId;
  post(id === undefined ? { cmd: 'set_brightness', percent } : { cmd: 'set_lamp_brightness', id, percent });
});
document.addEventListener('focusout', ev => {
  if (ev.target.id === 'popoverresetminutes' && view === 'settings') render();
  if (isBrightnessSlider(ev.target) && view === 'lamp') {
    if (previewPointer === null && previewSlider === ev.target) stopBrightnessPreview();
    renderLamp();
  }
});

document.addEventListener('click', ev => {
  const local = ev.target.closest('[data-local]');
  if (local) {
    if (view === 'lamp') stopBrightnessPreview();
    view = view === local.dataset.local ? 'sessions' : local.dataset.local;
    render();
    return;
  }
  const btn = ev.target.closest('[data-cmd]');
  if (!btn || btn.disabled) return;
  if (view === 'lamp') stopBrightnessPreview();
  const msg = { cmd: btn.dataset.cmd };
  if (btn.dataset.id !== undefined) msg.id = btn.dataset.id;
  if (btn.dataset.enabled !== undefined) msg.enabled = btn.dataset.enabled === 'true';
  if (msg.cmd === 'clear_session' || msg.cmd === 'clear_paused') {
    configActionError = '';
    document.getElementById('sessionerror').hidden = true;
  }
  if (lampSetupBusy() && ['add_lamp', 'forget_lamp', 'rename_lamp', 'identify_lamp', 'set_led', 'update_firmware'].includes(msg.cmd)) return;
  if (msg.cmd === 'update_firmware') {
    if (!canUpdateFirmware()) return;
    firmwarePending = true;
    firmwareError = firmwareMessage = '';
    firmwareProgress = 0;
    configActionError = '';
    renderLamp();
  }
  if (msg.cmd === 'identify_lamp') {
    if (!canIdentifyLamp(msg.id)) return;
    pendingIdentifyId = msg.id;
    configActionError = '';
    renderLamp();
  }
  if (msg.cmd === 'add_lamp') {
    if (state.lamp?.bluetooth_supported !== true || lampActionPending()) return;
    lampSetupPending = true;
    lampSetupError = lampSetupMessage = '';
    render();
  }
  if (msg.cmd === 'forget_lamp') {
    if (lampActionPending() || !msg.id) return;
    pendingForgetId = msg.id;
    configActionError = '';
    renderLamp();
  }
  if (msg.cmd === 'rename_lamp') {
    if (lampActionPending() || !msg.id) return;
    savedRenameId = null;
    const input = document.getElementById(btn.dataset.input);
    lampNameDrafts.set(msg.id, input.value);
    msg.name = input.value.trim();
    if ([...msg.name].length > 40) {
      configActionError = 'Use 40 characters or fewer for the lamp name.';
      render();
      input.focus();
      return;
    }
    pendingRenameId = msg.id;
    configActionError = '';
    renderLamp();
  }
  // Optimistic enable is pending, never a successful connection.
  if (msg.cmd === 'set_led') {
    state.lamp = state.lamp || {};
    state.lamp.enabled = msg.enabled;
    state.lamp.state = msg.enabled ? 'connecting' : 'disabled';
    state.lamp.devices = [];
    state.lamp.detail = msg.enabled ? 'Waiting for the service.' : 'Stopping lamp updates.';
    renderLamp();
  }
  post(msg);
});
document.addEventListener('keydown', ev => {
  if (ev.key === 'Enter' && !ev.isComposing && ev.target.id?.startsWith('lampname-')) {
    ev.preventDefault();
    document.getElementById(ev.target.id.replace('lampname-', 'lampsave-'))?.click();
    return;
  }
  if (ev.key !== 'Escape') return;
  stopBrightnessPreview();
  if (view !== 'sessions') { view = 'sessions'; render(); }
  else post({ cmd: 'hide' });
});
document.addEventListener('contextmenu', ev => ev.preventDefault());
setInterval(() => { if (state.sessions.length) render(); }, 30000);
render();
post({ cmd: 'ready' });
