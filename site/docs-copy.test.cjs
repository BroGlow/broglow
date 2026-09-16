const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');
const script = readFileSync(`${__dirname}/guide/assets/copy.js`, 'utf8');

async function check(lang, clipboard) {
  let click;
  const copied = [];
  const feedback = { textContent: '', classList: { contains: () => true } };
  const code = { textContent: ' prompt in the selected language ', closest: () => ({ nextElementSibling: feedback }) };
  const button = { dataset: { clipboardTarget: '#code' }, disabled: false };
  runInNewContext(script, {
    document: {
      documentElement: { lang },
      addEventListener(event, fn, capture) { assert.equal(event, 'click'); assert.equal(capture, true); click = fn; },
      querySelector(selector) { assert.equal(selector, '#code'); return code; },
    },
    navigator: { clipboard: clipboard === 'missing' ? undefined : { async writeText(text) {
      if (clipboard === 'denied') throw Error('denied');
      copied.push(text);
    } } },
  });
  await click({ target: { closest: () => null } });
  const copying = click({ target: { closest: () => button }, preventDefault() {}, stopImmediatePropagation() {} });
  await copying;
  assert.equal(button.disabled, false);
  if (clipboard === 'allowed') {
    assert.deepEqual(copied, ['prompt in the selected language']);
    assert.equal(feedback.textContent, lang === 'ru' ? 'Скопировано!' : 'Copied!');
  } else {
    assert.deepEqual(copied, []);
    assert.match(feedback.textContent, lang === 'ru' ? /вручную/ : /manually/);
  }
}
Promise.all(['en','ru'].flatMap((lang) => ['allowed','missing','denied'].map((mode) => check(lang, mode))))
  .then(() => console.log('PASS: guide copying and failure feedback in both languages'))
  .catch((error) => { console.error(error); process.exitCode = 1; });
