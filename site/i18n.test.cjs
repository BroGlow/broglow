const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { runInNewContext } = require("node:vm");
const script = readFileSync(`${__dirname}/i18n.js`, "utf8");

function load({ languages, language, saved, blocked = false } = {}) {
  const text = { textContent: "Working", "data-ru": "Работает" };
  const meta = { content: "Description", "data-ru-content": "Описание" };
  const image = { alt: "Render", "data-ru-alt": "Рендер" };
  const picker = { value: "en", hidden: true, "aria-label": "Language", "data-ru-aria-label": "Язык" };
  const link = { href: "docs/hardware/#checks" };
  const elements = [text, meta, image, picker];
  for (const element of [...elements, link]) {
    element.getAttribute = (key) => element[key];
    element.setAttribute = (key, value) => { element[key] = value; };
  }
  let change;
  picker.addEventListener = (event, callback) => {
    assert.equal(event, "change");
    change = callback;
  };
  const document = {
    documentElement: { lang: "en" },
    querySelector: (selector) => { assert.equal(selector, "#language"); return picker; },
    querySelectorAll: (selector) => selector === 'a[href^="docs/"]' ? [link]
      : elements.filter((element) => selector.slice(1, -1) in element),
    getElementById: (id) => elements.find((element) => element.id === id),
  };
  const storage = { saved };
  runInNewContext(script, {
    document,
    navigator: { languages, language },
    localStorage: {
      getItem(key) {
        assert.equal(key, "broglow-language");
        if (blocked) throw new Error("Storage blocked");
        return storage.saved;
      },
      setItem(key, value) {
        assert.equal(key, "broglow-language");
        if (blocked) throw new Error("Storage blocked");
        storage.saved = value;
      },
    },
  });
  assert.equal(picker.hidden, false);
  assert.equal(picker.value, document.documentElement.lang);
  return { document, text, meta, image, picker, link, storage, switchTo(value) { picker.value = value; change(); } };
}

for (const [options, expected] of [
  [{ languages: ["ru-RU", "en-US"] }, "en"],
  [{ languages: ["en-GB", "ru"] }, "en"],
  [{ languages: ["fr-FR", "ru-KZ"] }, "en"],
  [{ language: "RU-ru" }, "en"],
  [{ languages: ["de-DE"] }, "en"],
  [{}, "en"],
  [{ languages: ["en"], saved: "ru" }, "ru"],
  [{ languages: ["ru"], saved: "en" }, "en"],
  [{ languages: ["ru"], saved: "invalid" }, "en"],
]) assert.equal(load(options).document.documentElement.lang, expected);

const page = load({ languages: ["en-US"] });
assert.equal(page.storage.saved, undefined, "The default must not overwrite the preference");
assert.equal(page.text.textContent, "Working");
assert.equal(page.meta.content, "Description");
assert.equal(page.image.alt, "Render");
assert.equal(page.picker["aria-label"], "Language");
assert.equal(page.link.href, "docs/hardware/?lang=en#checks");
page.switchTo("ru");
assert.equal(page.document.documentElement.lang, "ru");
assert.equal(page.link.href, "docs/hardware/?lang=ru#checks");
assert.equal(page.text.textContent, "Работает");
assert.equal(page.meta.content, "Описание");
assert.equal(page.image.alt, "Рендер");
assert.equal(page.picker["aria-label"], "Язык");
assert.equal(load({ language: "en", saved: page.storage.saved }).picker.value, "ru");
page.switchTo("en");
assert.equal(page.text.textContent, "Working");
assert.equal(page.storage.saved, "en");

const privatePage = load({ language: "ru", blocked: true });
assert.equal(privatePage.document.documentElement.lang, "en");
privatePage.switchTo("ru");
assert.equal(privatePage.document.documentElement.lang, "ru");
assert.equal(privatePage.text.textContent, "Работает");
privatePage.switchTo("en");
assert.equal(privatePage.document.documentElement.lang, "en");
assert.equal(privatePage.text.textContent, "Working");

console.log("PASS: localization, saved preferences, and blocked storage");
