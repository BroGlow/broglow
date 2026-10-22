const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { resolve } = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const source = readFileSync(resolve(__dirname, 'ar.js'), 'utf8');

class DetailEvent extends Event {
  constructor(type, detail) { super(type); this.detail = detail; }
}

function preview({ href = 'https://example.com/project/index.html?private=secret&theme=dark#desk', mobile = false,
  canActivateAR = false, launchError = false, qrFailure = false, language = 'en' } = {}) {
  const document = { documentElement: { style: { overflow: 'auto' } } };
  function node() {
    const element = new EventTarget();
    element.hidden = true;
    element.isConnected = true;
    element.focus = () => { document.activeElement = element; };
    element.getBoundingClientRect = () => ({ left: 100, right: 500, top: 100, bottom: 600 });
    return element;
  }
  const nodes = Object.fromEntries(['lamp-model', 'model-space', 'model-ar', 'ar-dialog', 'ar-close',
    'ar-qr', 'ar-link', 'ar-public-link', 'ar-local', 'ar-qr-error', 'ar-reason', 'ar-scan'].map(id => [id, node()]));
  const viewer = nodes['lamp-model'];
  viewer.canActivateAR = canActivateAR;
  viewer.inert = false;
  let launches = 0;
  viewer.activateAR = () => {
    launches++;
    return launchError ? Promise.reject(Error('native viewer failed')) : Promise.resolve();
  };
  let scrolled = false;
  viewer.scrollIntoView = () => { scrolled = true; };
  const camera = node();
  viewer.shadowRoot = { querySelector: () => camera };
  const dialog = nodes['ar-dialog'];
  const pendingCloses = [];
  dialog.showModal = () => { dialog.open = true; };
  dialog.close = () => {
    if (!dialog.open) return;
    dialog.open = false;
    pendingCloses.push(new Promise(resolve => setImmediate(() => {
      dialog.dispatchEvent(new Event('close'));
      resolve();
    })));
  };
  dialog.cancel = () => {
    const event = new Event('cancel', { cancelable: true });
    dialog.dispatchEvent(event);
    if (!event.defaultPrevented) dialog.close();
  };
  document.querySelector = selector => nodes[selector.slice(1)];
  document.activeElement = nodes['model-space'];
  const browser = new EventTarget();
  browser.BroGlowI18n = { language, t: key => `${browser.BroGlowI18n.language}:${key}` };
  const active = [];
  browser.addEventListener('broglow:ar-active', event => active.push(event.detail.active));
  const qrValues = [];
  vm.runInNewContext(source, {
    document, window: browser, location: { href }, URL,
    navigator: { userAgent: mobile ? 'Mozilla/5.0 (iPhone)' : 'Desktop browser' },
    CustomEvent: class extends DetailEvent { constructor(type, options) { super(type, options.detail); } },
    qrcode: () => ({
      addData(value) { qrValues.push(value); },
      make() { if (qrFailure) throw Error('QR failed'); },
      createDataURL() { return 'data:image/gif;base64,QR'; },
    }),
  });
  return {
    nodes, document, browser, qrValues, active, camera,
    launches: () => launches, scrolled: () => scrolled,
    closed: () => Promise.all(pendingCloses),
    click(id, fields) {
      const event = new Event('click', { cancelable: true });
      Object.assign(event, fields);
      nodes[id].dispatchEvent(event);
      return event;
    },
    status(status) { viewer.dispatchEvent(new DetailEvent('ar-status', { status })); },
  };
}

test('desktop handoff retains the deployment path and language, omits unrelated query values', () => {
  const demo = preview({ canActivateAR: true, language: 'ru' });
  demo.click('model-space');
  assert.equal(demo.launches(), 0, 'desktop opens QR even if an AR mode is reported');
  assert.equal(demo.nodes['ar-dialog'].open, true);
  assert.equal(demo.nodes['ar-link'].href, 'https://example.com/project/index.html?lang=ru#view-in-space');
  assert.deepEqual(demo.qrValues, [demo.nodes['ar-link'].href]);
  assert.equal(demo.nodes['ar-qr'].hidden, false);
  assert.equal(demo.nodes['ar-local'].hidden, true);
  assert.equal(demo.nodes['ar-public-link'].hidden, true);
});

test('loopback and file previews never produce an unreachable QR', () => {
  for (const href of ['http://localhost:8766/demo/', 'http://test.localhost/demo/',
    'http://127.0.0.1/demo/', 'http://127.2.3.4/demo/', 'http://[::1]/demo/',
    'http://0.0.0.0/demo/', 'file:///project/demo/index.html']) {
    const demo = preview({ href });
    demo.click('model-space');
    assert.equal(demo.nodes['ar-qr'].hidden, true, href);
    assert.equal(demo.nodes['ar-local'].hidden, false, href);
    assert.equal(demo.nodes['ar-public-link'].hidden, false);
    assert.equal(demo.nodes['ar-public-link'].href, 'https://broglow.github.io/broglow/?lang=en#view-in-space');
    assert.deepEqual(demo.qrValues, []);
    assert.match(demo.nodes['ar-link'].href, /#view-in-space$/);
  }
});

test('supported phone launches directly from the click and native return resumes the demo', async () => {
  const demo = preview({ mobile: true, canActivateAR: true });
  demo.click('model-space');
  assert.equal(demo.launches(), 1, 'activation happens before the click call stack ends');
  assert.deepEqual(demo.active, [true]);
  assert.equal(demo.nodes['ar-dialog'].open, undefined);
  demo.status('session-started');
  demo.status('object-placed');
  demo.status('not-presenting');
  assert.deepEqual(demo.active, [true, false]);
  demo.click('model-space');
  demo.browser.dispatchEvent(new Event('focus'));
  assert.deepEqual(demo.active, [true, false, true, false]);
  demo.click('model-space');
  demo.browser.dispatchEvent(new Event('pageshow'));
  assert.equal(demo.active.at(-1), false);
  await Promise.resolve();
});

test('unavailable phone offers guidance; capability is rechecked after asynchronous readiness', async () => {
  const demo = preview({ mobile: true });
  demo.click('model-space');
  assert.equal(demo.launches(), 0);
  assert.equal(demo.nodes['ar-reason'].textContent, 'en:ar.unavailable');
  demo.nodes['ar-dialog'].close();
  await demo.closed();
  demo.nodes['lamp-model'].canActivateAR = true;
  demo.click('model-space');
  assert.equal(demo.launches(), 1);
});

test('resolved native handoffs resume without focus; presenting WebXR stays paused until its status ends', async () => {
  const demo = preview({ mobile: true, canActivateAR: true });
  demo.click('model-space');
  assert.deepEqual(demo.active, [true]);
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(demo.active, [true, false], 'an aborted native handoff cannot freeze scroll animation');
  demo.click('model-space');
  demo.status('session-started');
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(demo.active.at(-1), true, 'a resolved launch does not end a presenting WebXR session');
  demo.browser.dispatchEvent(new Event('focus'));
  demo.browser.dispatchEvent(new Event('pageshow'));
  assert.equal(demo.active.at(-1), true, 'page lifecycle events do not override presenting status');
  demo.status('object-placed');
  demo.status('not-presenting');
  assert.deepEqual(demo.active, [true, false, true, false]);
});

test('native failures stay separate from model loading and remain localized', async () => {
  const demo = preview({ mobile: true, canActivateAR: true, launchError: true });
  let modelErrors = 0;
  demo.browser.addEventListener('broglow:model-error', () => modelErrors++);
  demo.click('model-space');
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(demo.active, [true, false]);
  assert.equal(demo.nodes['ar-dialog'].open, true);
  assert.equal(demo.nodes['ar-reason'].textContent, 'en:ar.failed');
  demo.browser.BroGlowI18n.language = 'ru';
  demo.browser.dispatchEvent(new Event('broglow:language'));
  assert.equal(demo.nodes['ar-reason'].textContent, 'ru:ar.failed');
  assert.match(demo.qrValues.at(-1), /\?lang=ru#view-in-space$/);
  demo.nodes['ar-dialog'].close();
  await demo.closed();
  demo.status('failed');
  assert.equal(demo.nodes['ar-dialog'].open, true);
  assert.equal(demo.nodes['lamp-model'].inert, false);
  assert.equal(modelErrors, 0);
});

test('native slot launches once through the same guarded path and clears state on failure', () => {
  const demo = preview({ mobile: true, canActivateAR: true });
  demo.click('model-ar');
  assert.deepEqual(demo.active, [true]);
  assert.equal(demo.launches(), 1, 'slotted-button propagation cannot launch a second time');
  demo.status('failed');
  assert.deepEqual(demo.active, [true, false]);
  assert.equal(demo.nodes['ar-reason'].textContent, 'en:ar.failed');
});

test('a capability-gated native slot can launch desktop WebXR while the outer desktop button stays QR', () => {
  const demo = preview({ canActivateAR: true });
  demo.click('model-ar');
  assert.equal(demo.launches(), 1);
  assert.equal(demo.nodes['ar-dialog'].open, undefined);
  demo.status('not-presenting');
  demo.click('model-space');
  assert.equal(demo.launches(), 1, 'outer desktop button still opens the phone handoff');
  assert.equal(demo.nodes['ar-dialog'].open, true);
});

test('dialog locks only while open; close, Escape and backdrop restore focus and scrolling', async () => {
  const demo = preview();
  const dialog = demo.nodes['ar-dialog'];
  const opener = demo.nodes['model-space'];
  demo.click('model-space');
  assert.equal(demo.document.documentElement.style.overflow, 'hidden');
  assert.equal(demo.document.activeElement, demo.nodes['ar-close']);
  demo.click('ar-qr');
  demo.click('ar-dialog', { clientX: 150, clientY: 150 });
  assert.equal(dialog.open, true, 'content and padding remain clickable');
  demo.click('ar-dialog', { clientX: 50, clientY: 50 });
  await demo.closed();
  assert.equal(dialog.open, false);
  assert.equal(demo.document.documentElement.style.overflow, 'auto');
  assert.equal(demo.document.activeElement, opener);
  demo.click('model-space');
  dialog.cancel();
  await demo.closed();
  assert.equal(dialog.open, false, 'native Escape is not intercepted');
  assert.equal(demo.document.activeElement, opener);
  demo.click('model-space');
  demo.click('ar-close');
  await demo.closed();
  assert.equal(dialog.open, false);
  assert.equal(demo.document.documentElement.style.overflow, 'auto');
});

test('QR failure keeps an on-device link; it closes the dialog and focuses the healthy model', async () => {
  const demo = preview({ qrFailure: true });
  demo.click('model-space');
  assert.equal(demo.nodes['ar-qr'].hidden, true);
  assert.equal(demo.nodes['ar-qr-error'].hidden, false);
  assert.match(demo.nodes['ar-link'].href, /#view-in-space$/);
  const event = demo.click('ar-link');
  await demo.closed();
  assert.equal(event.defaultPrevented, true, 'same-device action preserves the current demo');
  assert.equal(demo.nodes['ar-dialog'].open, false);
  assert.equal(demo.scrolled(), true);
  assert.equal(demo.document.activeElement, demo.camera);
  demo.document.activeElement = demo.nodes['model-space'];
  demo.nodes['lamp-model'].inert = true;
  demo.click('model-space');
  demo.click('ar-link');
  await demo.closed();
  assert.equal(demo.document.activeElement, demo.nodes['model-space'], 'failed hidden model is not focused');
});
