const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { runInNewContext } = require("node:vm");
const script = readFileSync(`${__dirname}/i18n.js`, "utf8");

function load({ languages, language, saved, blocked = false } = {}) {
  const text = { textContent: "Работает", "data-en": "Working" };
  const meta = { content: "Описание", "data-en-content": "Description" };
  const image = { alt: "Рендер", "data-en-alt": "Render" };
  const picker = { value: "ru", hidden: true, "aria-label": "Язык", "data-en-aria-label": "Language" };
  const elements = [text, meta, image, picker];
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
    querySelectorAll: (selector) => elements.filter((element) => selector.slice(1, -1) in element),
  };
  const storage = { saved };
  runInNewContext(script, {
    document,
    navigator: { languages, language },
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
  return { document, text, meta, image, picker, storage, switchTo(value) { picker.value = value; change(); } };
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
console.log("PASS: browser locales, fallback, saved preference, manual switching, text/metadata/accessibility, blocked storage");
