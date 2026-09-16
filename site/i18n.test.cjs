const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { runInNewContext } = require("node:vm");
const script = readFileSync(`${__dirname}/i18n.js`, "utf8");

function load({ languages, language, saved, blocked = false, clipboard = "allowed" } = {}) {
  const text = { textContent: "Работает", "data-en": "Working" };
  const meta = { content: "Описание", "data-en-content": "Description" };
  const image = { alt: "Рендер", "data-en-alt": "Render" };
  const picker = { value: "ru", hidden: true, "aria-label": "Язык", "data-en-aria-label": "Language" };
  const prompts = [
    { id: "setup-prompt", textContent: " Настрой лампу. ", "data-en": " Set up my lamp. " },
    { id: "dark-prompt", textContent: " Почему лампа не светится? ", "data-en": " Why is my lamp dark? " },
  ];
  const copies = prompts.map((prompt) => {
    const success = { hidden: true };
    const error = { hidden: true };
    const button = {
      "data-copy": prompt.id,
      dataset: { copy: prompt.id },
      hidden: true,
      parentElement: { querySelector: (selector) => selector === "[data-copy-success]" ? success : error },
      addEventListener(event, callback) { assert.equal(event, "click"); this.click = callback; },
    };
    return { button, success, error };
  });
  const feedback = copies.flatMap(({ success, error }) => [success, error]);
  const elements = [text, meta, image, picker, ...prompts, ...copies.map(({ button }) => button)];
  for (const element of elements) {
    element.getAttribute = (key) => element[key];
    element.setAttribute = (key, value) => { element[key] = value; };
  }
  let change;
  picker.addEventListener = (event, callback) => {
    assert.equal(event, "change");
    change = callback;
  };
  const document = {
    documentElement: { lang: "ru" },
    querySelector: (selector) => { assert.equal(selector, "#language"); return picker; },
    querySelectorAll: (selector) => selector === ".copy-feedback" ? feedback : elements.filter((element) => selector.slice(1, -1) in element),
    getElementById: (id) => elements.find((element) => element.id === id),
  };
  const storage = { saved };
  const copied = [];
  runInNewContext(script, {
    document,
    navigator: { languages, language, clipboard: clipboard === "missing" ? undefined : {
      async writeText(value) {
        if (clipboard === "denied") throw new Error("Clipboard denied");
        copied.push(value);
      },
    } },
    localStorage: {
      getItem(key) {
        assert.equal(key, "clawlight-language");
        if (blocked) throw new Error("Storage blocked");
        return storage.saved;
      },
      setItem(key, value) {
        assert.equal(key, "clawlight-language");
        if (blocked) throw new Error("Storage blocked");
        storage.saved = value;
      },
    },
  });
  assert.equal(picker.hidden, false);
  assert.equal(picker.value, document.documentElement.lang);
  return { document, text, meta, image, picker, storage, copies, copied, switchTo(value) { picker.value = value; change(); } };
}

for (const [options, expected] of [
  [{ languages: ["ru-RU", "en-US"] }, "ru"],
  [{ languages: ["en-GB", "ru"] }, "en"],
  [{ languages: ["fr-FR", "ru-KZ"] }, "ru"],
  [{ language: "RU-ru" }, "ru"],
  [{ languages: ["de-DE"] }, "en"],
  [{}, "en"],
  [{ languages: ["en"], saved: "ru" }, "ru"],
  [{ languages: ["ru"], saved: "en" }, "en"],
  [{ languages: ["ru"], saved: "invalid" }, "ru"],
]) assert.equal(load(options).document.documentElement.lang, expected);

const page = load({ languages: ["en-US"] });
assert.equal(page.storage.saved, undefined, "Auto-detection must not overwrite the preference");
assert.equal(page.text.textContent, "Working");
assert.equal(page.meta.content, "Description");
assert.equal(page.image.alt, "Render");
assert.equal(page.picker["aria-label"], "Language");
page.switchTo("ru");
assert.equal(page.document.documentElement.lang, "ru");
assert.equal(page.text.textContent, "Работает");
assert.equal(page.meta.content, "Описание");
assert.equal(page.image.alt, "Рендер");
assert.equal(page.picker["aria-label"], "Язык");
assert.equal(load({ language: "en", saved: page.storage.saved }).picker.value, "ru");
page.switchTo("en");
assert.equal(page.text.textContent, "Working");
assert.equal(page.storage.saved, "en");

const privatePage = load({ language: "ru", blocked: true });
privatePage.switchTo("en");
assert.equal(privatePage.document.documentElement.lang, "en");
assert.equal(privatePage.text.textContent, "Working");

async function checkCopy() {
  for (const copy of page.copies) {
    assert.equal(copy.button.hidden, false);
    const copying = copy.button.click();
    assert.equal(copy.button.disabled, true);
    await copying;
    assert.equal(copy.button.disabled, false);
    assert.equal(copy.success.hidden, false);
    assert.equal(copy.error.hidden, true);
  }
  assert.deepEqual(page.copied, ["Set up my lamp.", "Why is my lamp dark?"]);
  page.switchTo("ru");
  assert.ok(page.copies.every(({ success, error }) => success.hidden && error.hidden));
  await page.copies[0].button.click();
  assert.equal(page.copied.at(-1), "Настрой лампу.", "Copy the current language, not cached text");
  for (const clipboard of ["denied", "missing"]) {
    const unavailable = load({ language: "en", clipboard });
    const copy = unavailable.copies[0];
    await copy.button.click();
    assert.equal(copy.success.hidden, true);
    assert.equal(copy.error.hidden, false);
    assert.equal(copy.button.disabled, false);
    assert.deepEqual(unavailable.copied, []);
  }
}
checkCopy().then(() => console.log("PASS: localization, saved preferences, both prompts, current-language copying, clipboard failure fallback"))
  .catch((error) => { console.error(error); process.exitCode = 1; });
