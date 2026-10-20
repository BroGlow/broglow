const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { test } = require("node:test");
const { runInNewContext } = require("node:vm");
const script = readFileSync(`${__dirname}/github.js`, "utf8");

async function load(fetch, href = "https://github.com/broglow/broglow") {
  const nodes = {
    "[data-stars]": { textContent: "" },
    "[data-forks]": { textContent: "" },
    ".repository-description": { hidden: false },
    ".repository-stats": { hidden: true },
  };
  const badge = { href, querySelector: selector => nodes[selector] };
  await runInNewContext(script, {
    document: { querySelector: selector => {
      assert.equal(selector, ".repository-badge");
      return badge;
    } },
    URL, fetch,
  });
  return { badge, nodes };
}

test("loads stars and forks for the rendered repository, including zero", async () => {
  const { nodes } = await load(async url => {
    assert.equal(url, "https://api.github.com/repos/example/lamp");
    return { ok: true, json: async () => ({ stargazers_count: 12345, forks_count: 0 }) };
  }, "https://github.com/example/lamp");
  assert.equal(nodes["[data-stars]"].textContent, "12,345");
  assert.equal(nodes["[data-forks]"].textContent, "0");
  assert.equal(nodes[".repository-description"].hidden, true);
  assert.equal(nodes[".repository-stats"].hidden, false);
});

test("failed or invalid responses leave the source link usable without false counts", async () => {
  for (const fetch of [
    async () => { throw Error("Offline"); },
    async () => ({ ok: false }),
    async () => ({ ok: true, json: async () => { throw Error("Invalid JSON"); } }),
    ...[{}, null, { stargazers_count: -1, forks_count: 0 }, { stargazers_count: "5", forks_count: 1 }]
      .map(counts => async () => ({ ok: true, json: async () => counts })),
  ]) {
    const { badge, nodes } = await load(fetch);
    assert.equal(badge.href, "https://github.com/broglow/broglow");
    assert.equal(nodes["[data-stars]"].textContent, "");
    assert.equal(nodes["[data-forks]"].textContent, "");
    assert.equal(nodes[".repository-description"].hidden, false);
    assert.equal(nodes[".repository-stats"].hidden, true);
  }
});
