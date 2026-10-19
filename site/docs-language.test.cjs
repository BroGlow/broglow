const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');
const script = readFileSync(`${__dirname}/guide/assets/language.js`, 'utf8');

function check({ current = 'en', saved, explicit, languages = ['en-US'], blocked = false } = {}) {
  const links = ['en', 'ru'].map((hreflang) => ({ hreflang, href: `https://example.test/site/docs/${hreflang === 'ru' ? 'ru/' : ''}codex/`, addEventListener(event, fn) { assert.equal(event, 'click'); this.click = fn; } }));
  const state = { saved, redirect: null };
  const location = { href: `https://example.test/site/docs/${current === 'ru' ? 'ru/' : ''}codex/${explicit ? `?lang=${explicit}` : ''}#codex-setup`, hash: '#codex-setup', replace(value) { state.redirect = value; }, assign(value) { state.navigation = value; } };
  runInNewContext(script, {
    URL,
    location,
    document: { documentElement: { lang: current }, querySelectorAll(selector) { assert.equal(selector, 'a[hreflang]'); return links; } },
    navigator: { languages },
    localStorage: {
      getItem(key) { assert.equal(key, 'broglow-language'); if (blocked) throw Error('blocked'); return saved; },
      setItem(key, value) { assert.equal(key, 'broglow-language'); if (blocked) throw Error('blocked'); state.saved = value; },
    },
  });
  return Object.assign(state, { links, setHash(value) { location.hash = value; } });
}

assert.equal(check({ languages: ['ru-RU'] }).redirect, null);
assert.equal(check({ current: 'ru', languages: ['en'] }).redirect, null);
assert.match(check({ saved: 'ru', languages: ['en'] }).redirect, /\/ru\/codex\/#codex-setup$/);
assert.equal(check({ languages: ['ru'], blocked: true }).redirect, null);
assert.match(check({ current: 'ru', saved: 'en', languages: ['ru'] }).redirect, /\/docs\/codex\/#codex-setup$/);
assert.equal(check({ saved: 'en', languages: ['ru'] }).redirect, null);
assert.equal(check({ languages: ['de'], saved: 'invalid' }).redirect, null);
assert.equal(check({ languages: ['ru'] }).saved, undefined, 'The default must not save a manual preference');
const manual = check({ explicit: 'en', saved: 'ru', languages: ['ru'] });
assert.equal(manual.saved, 'en');
assert.equal(manual.redirect, null);
assert.equal(check({ explicit: 'en', saved: 'ru', languages: ['ru'], blocked: true }).redirect, null);
assert.ok(manual.links.every((link) => new URL(link.href).searchParams.get('lang') === link.hreflang));
assert.ok(manual.links.every((link) => new URL(link.href).hash === '#codex-setup'));
manual.setHash('#pairing');
manual.links[1].click({ preventDefault() {}, stopImmediatePropagation() {} });
assert.equal(manual.saved, 'ru', 'Save before the theme handles navigation');
assert.equal(new URL(manual.navigation).hash, '#pairing');
assert.equal(new URL(manual.navigation).searchParams.get('lang'), 'ru');
assert.match(check({ explicit: 'ru', blocked: true }).redirect, /\/ru\/codex\/\?lang=ru#codex-setup$/);
assert.equal(check({ current: 'ru', explicit: 'ru', blocked: true }).redirect, null);
console.log('PASS: docs URL language, saved and manual choices, anchors, and blocked storage');
