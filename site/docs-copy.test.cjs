const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { test } = require('node:test');
const { runInNewContext } = require('node:vm');
const script = readFileSync(`${__dirname}/guide/assets/copy.js`, 'utf8');

function page(lang, clipboard) {
  let click;
  const pre = { after(element) { this.nextElementSibling = element; } };
  const code = { textContent: '  Prompt in the selected language\n', closest: () => pre };
  const button = { dataset: { clipboardTarget: '#code' }, disabled: false };
  runInNewContext(script, {
    document: {
      documentElement: { lang },
      addEventListener(event, fn) { if (event === 'click') click = fn; },
      querySelector: () => code,
      createElement() {
        return {
          textContent: '',
          setAttribute(name, value) { this[name] = value; },
          classList: { contains: name => pre.nextElementSibling.className === name },
        };
      },
    },
    navigator: { clipboard },
  });
  return {
    button,
    get feedback() { return pre.nextElementSibling; },
    copy() {
      return click({ target: { closest: () => button }, preventDefault() {}, stopImmediatePropagation() {} });
    },
  };
}

for (const [lang, message] of [['en', 'Copied!'], ['ru', 'Скопировано!']]) {
  test(`copies the prompt and announces success in ${lang}`, async () => {
    const copied = [];
    let finish;
    const clipboard = { writeText(text) {
      copied.push(text);
      return new Promise(resolve => { finish = resolve; });
    } };
    const guide = page(lang, clipboard);

    const pending = guide.copy();
    assert.equal(guide.button.disabled, true, 'Prevent another copy while the clipboard is busy');
    finish();
    await pending;

    assert.deepEqual(copied, ['Prompt in the selected language']);
    assert.equal(guide.button.disabled, false);
    assert.equal(guide.feedback.role, 'status', 'Announce feedback to assistive technology');
    assert.equal(guide.feedback.textContent, message);
    const feedback = guide.feedback;
    const retry = guide.copy();
    finish();
    await retry;
    assert.equal(guide.feedback, feedback, 'Repeated copying reuses the feedback region');
  });
}

for (const [name, lang, clipboard, message] of [
  ['clipboard unavailable', 'en', undefined, /manually/],
  ['clipboard permission denied', 'ru', { async writeText() { throw Error('denied'); } }, /вручную/],
]) {
  test(`${name} offers manual copying and restores the button`, async () => {
    const guide = page(lang, clipboard);

    await guide.copy();

    assert.equal(guide.button.disabled, false);
    assert.equal(guide.feedback.role, 'status');
    assert.match(guide.feedback.textContent, message);
  });
}
