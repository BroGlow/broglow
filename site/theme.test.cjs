const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { runInNewContext } = require("node:vm");
const script = readFileSync(`${__dirname}/theme.js`, "utf8");

function load({ saved, light = false, blocked = false } = {}) {
  const root = { dataset: {} };
  const picker = { value: "" };
  const meta = { content: "#181a19" };
  const listeners = {};
  const media = { matches: light, addEventListener: (event, fn) => { assert.equal(event, "change"); listeners.media = fn; } };
  const storage = { saved };
  runInNewContext(script, {
    document: {
      documentElement: root,
      addEventListener: (event, fn) => { assert.equal(event, "DOMContentLoaded"); listeners.ready = fn; },
      querySelector: (selector) => selector === "#theme" ? picker : meta,
    },
    window: { matchMedia: (query) => { assert.equal(query, "(prefers-color-scheme: light)"); return media; } },
    localStorage: {
      getItem(key) { assert.equal(key, "broglow-theme"); if (blocked) throw new Error("blocked"); return storage.saved; },
      setItem(key, value) { assert.equal(key, "broglow-theme"); if (blocked) throw new Error("blocked"); storage.saved = value; },
    },
  });
  picker.addEventListener = (event, fn) => { assert.equal(event, "change"); listeners.change = fn; };
  listeners.ready();
  return { root, picker, meta, storage, media, listeners, switchTo(value) { picker.value = value; listeners.change(); } };
}

assert.equal(load().root.dataset.theme, "dark", "Dark system preference stays dark");
assert.equal(load({ light: true }).root.dataset.theme, "light", "Light system preference is followed");
assert.equal(load({ saved: "light" }).root.dataset.theme, "light");
assert.equal(load({ saved: "dark", light: true }).root.dataset.theme, "dark");
assert.equal(load({ saved: "weird", light: true }).root.dataset.theme, "light", "Unknown choices fall back to auto");
const page = load({ light: true });
assert.equal(page.picker.value, "auto");
assert.equal(page.storage.saved, undefined, "Auto-detection must not save a preference");
const lightChrome = page.meta.content;
page.switchTo("dark");
assert.equal(page.root.dataset.theme, "dark");
assert.equal(page.root.dataset.themeChoice, "dark", "The picker icon follows the choice");
assert.notEqual(page.meta.content, lightChrome, "Browser chrome changes with the theme");
assert.equal(page.storage.saved, "dark");
page.switchTo("auto");
assert.equal(page.root.dataset.theme, "light");
assert.equal(page.root.dataset.themeChoice, "auto");
assert.equal(page.meta.content, lightChrome, "Returning to light restores its browser chrome");
page.media.matches = false;
page.listeners.media();
assert.equal(page.root.dataset.theme, "dark", "System changes apply while auto is selected");
const privatePage = load({ blocked: true, light: true });
privatePage.switchTo("dark");
assert.equal(privatePage.root.dataset.theme, "dark");
console.log("PASS: theme auto/light/dark, saved choice, system changes and blocked storage");
