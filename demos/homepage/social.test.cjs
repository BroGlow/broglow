const assert = require('node:assert/strict');
const {readFileSync} = require('node:fs');
const {resolve} = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const html = readFileSync(resolve(__dirname, 'index.html'), 'utf8');
const head = html.match(/<head>([\s\S]*?)<\/head>/)[1];
const attributes = tag => Object.fromEntries([...tag.matchAll(/([\w:-]+)="([^"]*)"/g)].map(([, name, value]) => [name, value]));
const tags = [...head.matchAll(/<(?:meta|link)\b[^>]*>/g)].map(([tag]) => attributes(tag));
const meta = key => tags.find(tag => tag.property === key || tag.name === key)?.content;
const canonical = tags.find(tag => tag.rel === 'canonical')?.href;

test('social crawlers receive complete, matching metadata without executing JavaScript', () => {
  const title = head.match(/<title[^>]*>([^<]+)<\/title>/)[1];
  assert.equal(meta('og:type'), 'website');
  assert.equal(meta('og:site_name'), 'BroGlow');
  assert.equal(meta('og:locale'), 'en_US');
  assert.equal(meta('og:locale:alternate'), 'ru_RU');
  assert.equal(meta('twitter:card'), 'summary_large_image');
  for (const prefix of ['og', 'twitter']) {
    assert.equal(meta(`${prefix}:title`), title);
    assert.equal(meta(`${prefix}:description`), meta('description'));
    assert.equal(meta(`${prefix}:image`), meta('og:image'));
    assert.equal(meta(`${prefix}:image:alt`), meta('og:image:alt'));
    assert.ok(meta(`${prefix}:image:alt`).length > 30);
  }
});

test('canonical and social image use final HTTPS URLs and the existing photo dimensions', () => {
  assert.equal(canonical, 'https://broglow.github.io/broglow/');
  assert.equal(meta('og:url'), canonical);
  assert.equal(meta('og:image'), `${canonical}assets/photos/broglow-all-lights.jpg`);
  for (const url of [canonical, meta('og:url'), meta('og:image'), meta('twitter:image')]) {
    assert.equal(new URL(url).protocol, 'https:');
    assert.doesNotMatch(url, /localhost|127\.0\.0\.1|outputs\//);
  }
  assert.equal(meta('og:image:type'), 'image/jpeg');
  const photo = readFileSync(resolve(__dirname, '../../site/assets/photos/broglow-all-lights.jpg'));
  assert.equal(photo.readUInt16BE(0), 0xffd8);
  let dimensions;
  for (let offset = 2; offset < photo.length;) {
    assert.equal(photo[offset], 0xff);
    const marker = photo[offset + 1];
    const length = photo.readUInt16BE(offset + 2);
    if ([0xc0, 0xc1, 0xc2].includes(marker)) {
      dimensions = {width: photo.readUInt16BE(offset + 7), height: photo.readUInt16BE(offset + 5)};
      break;
    }
    offset += length + 2;
  }
  assert.deepEqual(dimensions, {width: Number(meta('og:image:width')), height: Number(meta('og:image:height'))});
  assert.ok(dimensions.width >= 1200);
});

test('language switching updates title, descriptions, image alternatives and locale together', () => {
  const nodes = tags.filter(tag => tag['data-i18n-content']).map(tag => ({
    attributes: {...tag},
    getAttribute(name) {return this.attributes[name];},
    setAttribute(name, value) {this.attributes[name] = value;},
  }));
  const title = {dataset: {i18n: 'page.title'}, textContent: ''};
  const root = {};
  const browser = {dispatchEvent() {}};
  vm.runInNewContext(readFileSync(resolve(__dirname, 'language.js'), 'utf8'), {
    document: {
      documentElement: root,
      querySelector() {return null;},
      querySelectorAll(selector) {return selector === '[data-i18n-content]' ? nodes : selector === '[data-i18n]' ? [title] : [];},
    },
    window: browser, URL, URLSearchParams,
    location: {search: '?lang=en', href: 'https://broglow.github.io/broglow/?lang=en'},
    localStorage: {getItem() {return null;}, setItem() {}},
    history: {replaceState() {}},
    CustomEvent: class {constructor(type, options) {this.type = type;this.detail = options.detail;}},
  });
  const rendered = key => nodes.find(node => node.attributes.property === key || node.attributes.name === key)?.attributes.content;
  for (const language of ['ru', 'en']) {
    browser.BroGlowI18n.setLanguage(language);
    assert.equal(root.lang, language);
    assert.equal(rendered('og:locale'), language === 'ru' ? 'ru_RU' : 'en_US');
    assert.equal(rendered('og:locale:alternate'), language === 'ru' ? 'en_US' : 'ru_RU');
    for (const prefix of ['og', 'twitter']) {
      assert.equal(rendered(`${prefix}:title`), title.textContent);
      assert.equal(rendered(`${prefix}:description`), rendered('description'));
      assert.equal(rendered(`${prefix}:image:alt`), browser.BroGlowI18n.t('desk.photoAlt'));
    }
  }
});
