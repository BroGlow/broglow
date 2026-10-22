const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const source = readFileSync(join(__dirname, 'theme.js'), 'utf8');

function appearance({ query = '', stored = {}, dark = false, blockedStorage = false } = {}) {
  const root = { dataset: {}, style: {} };
  const storage = new Map(Object.entries(stored));
  const events = [];
  const select = new EventTarget();
  select.value = 'auto';
  const system = new EventTarget();
  system.matches = dark;
  const browser = new EventTarget();
  browser.location = new URL(`https://broglow.test/demo/${query}#model-demo`);
  browser.matchMedia = () => system;
  browser.history = { replaceState(_state, _unused, url) { browser.location = new URL(url); } };
  browser.addEventListener('broglow:appearance', event => events.push(event.detail));
  vm.runInNewContext(source, {
    document: { documentElement: root, readyState: 'complete', querySelector: () => select },
    window: browser, URL, URLSearchParams,
    localStorage: {
      getItem(key) { if (blockedStorage) throw new Error('Storage disabled'); return storage.get(key); },
      setItem(key, value) { if (blockedStorage) throw new Error('Storage disabled'); storage.set(key, value); },
    },
    CustomEvent: class extends Event { constructor(type, init) { super(type); this.detail = init.detail; } },
  });
  return {
    root, storage, events, browser,
    theme(value) { select.value = value; select.dispatchEvent(new Event('change')); },
    system(value) { system.matches = value; system.dispatchEvent(new Event('change')); },
  };
}

test('query theme overrides the saved appearance without changing storage', () => {
  const page = appearance({ query:'?palette=ember&theme=light&lang=ru', stored:{'broglow-theme':'dark','broglow-demo-palette':'plum'} });
  assert.equal(page.root.dataset.palette, 'ember');
  assert.equal(page.root.dataset.theme, 'light');
  assert.equal(page.root.style.colorScheme, 'light');
  assert.equal(page.storage.get('broglow-demo-palette'), 'plum');
  assert.equal(page.storage.get('broglow-theme'), 'dark');
});

test('system appearance follows live OS changes; explicit appearance does not', () => {
  const page = appearance({ dark:true });
  assert.equal(page.root.dataset.themePreference, 'auto');
  assert.equal(page.root.dataset.theme, 'dark');
  page.system(false);
  assert.equal(page.root.dataset.theme, 'light');
  page.theme('light');
  page.system(true);
  assert.equal(page.root.dataset.theme, 'light');
  page.theme('auto');
  assert.equal(page.root.dataset.theme, 'dark');
  assert.deepEqual(JSON.parse(JSON.stringify(page.events.at(-1))), { theme:'dark', palette:'ember' });
});

test('user changes persist the chosen preference and preserve language and anchor in share URLs', () => {
  const page = appearance({ query:'?lang=ru&palette=tide' });
  page.theme('dark');
  assert.equal(page.storage.get('broglow-theme'), 'dark');
  assert.equal(page.browser.location.searchParams.get('lang'), 'ru');
  assert.equal(page.browser.location.searchParams.has('palette'), false);
  assert.equal(page.browser.location.searchParams.get('theme'), 'dark');
  assert.equal(page.browser.location.hash, '#model-demo');
});

test('invalid inputs fall back to saved choices and unavailable storage does not block controls', () => {
  const saved = appearance({ query:'?palette=not-a-palette&theme=invalid', stored:{'broglow-theme':'light','broglow-demo-palette':'plum'} });
  assert.equal(saved.root.dataset.palette, 'ember');
  assert.equal(saved.root.dataset.theme, 'light');
  const page = appearance({ blockedStorage:true });
  page.theme('dark');
  assert.equal(page.root.dataset.palette, 'ember');
  assert.equal(page.root.dataset.theme, 'dark');
});

test('Ember stays fixed despite stale palette query parameters and stored preferences', () => {
  assert.equal(appearance().root.dataset.palette, 'ember');
  for (const palette of ['sage', 'ember', 'tide', 'plum', 'unknown']) {
    const page = appearance({ query:`?palette=${palette}`, stored:{'broglow-demo-palette':palette} });
    assert.equal(page.root.dataset.palette, 'ember');
    assert.equal(page.storage.get('broglow-demo-palette'), palette, 'obsolete storage is ignored without mutation');
    assert.deepEqual(JSON.parse(JSON.stringify(page.events.at(-1))), { theme:'light', palette:'ember' });
  }
});

test('Ember light and dark appearances meet AA text contrast', () => {
  const css = readFileSync(join(__dirname, 'themes.css'), 'utf8');
  function luminance(hex) {
    const rgb = [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16) / 255)
      .map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
    return rgb.reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
  }
  const contrast = (foreground, background) => {
    const [low, high] = [foreground, background].map(luminance).sort((a, b) => a - b);
    return (high + 0.05) / (low + 0.05);
  };
  let combinations = 0;
  for (const [, selector, body] of css.matchAll(/([^{}]+)\{([^{}]+)\}/g)) {
    const tokens = Object.fromEntries([...body.matchAll(/--([\w-]+):(#[0-9a-fA-F]{6})\b/g)].map(match => [match[1], match[2]]));
    if (!tokens.paper) continue;
    combinations += 1;
    for (const [text, surface] of [['ink', 'paper'], ['muted', 'paper'], ['accent', 'paper'], ['muted', 'surface']]) {
      assert.ok(contrast(tokens[text], tokens[surface]) >= 4.5, `${selector}: ${text} on ${surface}`);
    }
  }
  assert.equal(combinations, 2);
});
