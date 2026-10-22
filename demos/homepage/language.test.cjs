const assert = require('node:assert/strict');
const {readFileSync} = require('node:fs');
const {resolve} = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const script = readFileSync(resolve(__dirname, 'language.js'), 'utf8');
const html = readFileSync(resolve(__dirname, 'index.html'), 'utf8');

function preview({query = '', saved, storageBlocked = false} = {}) {
  const writes = [];
  const events = [];
  const makeNode = (key, attributes = {}) => ({
    dataset: {i18n: key}, textContent: '', attributes,
    getAttribute(name) {return this.attributes[name];},
    setAttribute(name, value) {this.attributes[name] = value;},
  });
  const heading = makeNode('hero.titleFirst');
  const title = makeNode('page.title');
  const label = makeNode(null, {'data-i18n-aria-label': 'tray.toggle'});
  const description = makeNode(null, {'data-i18n-content': 'page.description'});
  const alt = makeNode(null, {'data-i18n-alt': 'desk.photoAlt'});
  const frame = makeNode(null, {'data-i18n-title': 'app.title'});
  const guide = {href: 'https://broglow.github.io/broglow/docs/installation/#hooks'};
  const picker = new EventTarget();
  const document = {
    documentElement: {},
    querySelector(selector) {return selector === '#language' ? picker : null;},
    querySelectorAll(selector) {
      return ({'[data-i18n]': [heading, title], '[data-i18n-aria-label]': [label], '[data-i18n-alt]': [alt], '[data-i18n-title]': [frame], '[data-i18n-content]': [description], '[data-guide-link]': [guide]})[selector] ?? [];
    },
  };
  const location = {search: query, href: `http://localhost/index.html${query}#model-demo`};
  const listeners = {};
  const browser = {
    addEventListener(type, listener) {(listeners[type] ??= []).push(listener);},
    dispatchEvent(event) {events.push(event);for (const listener of listeners[event.type] ?? []) listener(event);},
  };
  vm.runInNewContext(script, {
    document, location, URL, URLSearchParams, window: browser,
    localStorage: {
      getItem(key) {assert.equal(key, 'broglow-language');if (storageBlocked) throw Error('storage blocked');return saved;},
      setItem(key, value) {if (storageBlocked) throw Error('storage blocked');writes.push([key, value]);},
    },
    history: {replaceState(_state, _title, url) {location.href = String(url);location.search = url.search;}},
    CustomEvent: class {constructor(type, options) {this.type = type;this.detail = options.detail;}},
  });
  return {api: browser.BroGlowI18n, document, heading, title, label, description, alt, frame, guide, picker, events, writes, location, emit: type => browser.dispatchEvent({type})};
}

test('query language overrides the saved choice without changing storage', () => {
  const demo = preview({query: '?lang=en', saved: 'ru'});
  assert.equal(demo.api.language, 'en');
  assert.equal(demo.document.documentElement.lang, 'en');
  assert.equal(demo.heading.textContent, 'Stay in flow.');
  assert.deepEqual(demo.writes, []);
  assert.equal(demo.events.at(-1).type, 'broglow:language');
  assert.equal(demo.events.at(-1).detail.language, 'en');
});

test('saved Russian localizes metadata, accessible labels, guide URLs', () => {
  const demo = preview({saved: 'ru'});
  assert.equal(demo.picker.value, 'ru');
  assert.equal(demo.title.textContent, demo.api.t('page.title'));
  assert.match(demo.title.textContent, /Ты в потоке/);
  assert.match(demo.description.attributes.content, /кодинг-агентов/);
  assert.match(demo.label.attributes['aria-label'], /панель сессий/);
  assert.match(demo.alt.attributes.alt, /Оранжевый/);
  assert.equal(demo.frame.attributes.title, 'Демо приложения BroGlow');
  assert.equal(demo.guide.href, 'https://broglow.github.io/broglow/docs/ru/installation/?lang=ru#hooks');
  demo.api.setLanguage('en');
  assert.equal(demo.guide.href, 'https://broglow.github.io/broglow/docs/installation/?lang=en#hooks');
  assert.equal(demo.document.documentElement.lang, 'en');
});

test('blocked storage still permits explicit and user-selected languages', () => {
  const demo = preview({query: '?lang=ru&theme=light', storageBlocked: true});
  assert.equal(demo.api.language, 'ru');
  demo.picker.value = 'en';
  demo.picker.dispatchEvent(new Event('change'));
  assert.equal(demo.api.language, 'en');
  assert.equal(new URL(demo.location.href).searchParams.get('theme'), 'light');
  assert.equal(new URL(demo.location.href).searchParams.get('lang'), 'en');
  assert.equal(new URL(demo.location.href).hash, '#model-demo');
  assert.deepEqual(demo.writes, []);
});

test('unsupported choices fall back safely; explicit selector changes are saved', () => {
  const demo = preview({query: '?lang=unknown', saved: 'unknown'});
  assert.equal(demo.api.language, 'en');
  demo.api.setLanguage('ru');
  assert.deepEqual(demo.writes, [['broglow-language', 'ru']]);
  demo.api.setLanguage('arbitrary');
  assert.equal(demo.api.language, 'ru');
  assert.equal(demo.writes.length, 1);
});

test('dictionary parity covers every annotated leaf and dynamic app message', () => {
  const {api} = preview();
  assert.deepEqual(Object.keys(api.dictionaries.en).sort(), Object.keys(api.dictionaries.ru).sort());
  for (const [, key] of html.matchAll(/data-i18n(?:-(?:aria-label|alt|title|content))?="([^"]+)"/g)) {
    assert.ok(api.dictionaries.en[key], `English translation for ${key}`);
    assert.ok(api.dictionaries.ru[key], `Russian translation for ${key}`);
  }
  const app = readFileSync(resolve(__dirname, 'app.js'), 'utf8');
  for (const [, key] of app.matchAll(/(?:text|line|announce)\('([^']+)'/g)) {
    assert.ok(api.dictionaries.en[key], `English dynamic translation for ${key}`);
    assert.ok(api.dictionaries.ru[key], `Russian dynamic translation for ${key}`);
  }
  api.setLanguage('ru');
  assert.equal(api.t('model.status', {state: 'Пауза', brightness: 42}), 'Пауза · яркость 42%');
  delete api.dictionaries.ru['state.working'];
  assert.equal(api.t('state.working'), 'Working');
  assert.equal(api.t('unknown.key'), 'unknown.key');
});

test('translation targets are text leaves so decorations, commands and structure survive', () => {
  let count = 0;
  for (const [, tag, content] of html.matchAll(/<([\w-]+)\b[^>]*\bdata-i18n="[^"]+"[^>]*>([\s\S]*?)<\/\1>/g)) {
    assert.doesNotMatch(content, /<[a-z]/i, `${tag} translation would erase a child element`);
    count++;
  }
  assert.ok(count > 90, 'all authored sections contain localizable text leaves');
  assert.match(html, /data-i18n="faq.lampQuestion"[^>]*>[^<]+<\/bdi><span aria-hidden="true">\+<\/span>/);
  const commands = html.match(/<pre><code>([\s\S]*?)<\/code><\/pre>/)[1].replace(/<[^>]+>/g, '');
  assert.equal(commands, 'brew install broglow/tap/broglow\nbroglow install');
  assert.match(html, /data-guide-link[^>]+data-i18n="footer.docs"/);
});
