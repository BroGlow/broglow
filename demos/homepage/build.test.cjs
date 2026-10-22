const assert = require('node:assert/strict');
const { mkdtemp, readFile, readdir, rm, writeFile } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { resolve } = require('node:path');
const test = require('node:test');
const { build } = require('./build.cjs');

async function fixture(t) {
  const directory = await mkdtemp(resolve(tmpdir(), 'broglow-static-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  return directory;
}

test('standalone build contains all page and dynamic product dependencies', async t => {
  const directory = await fixture(t);
  const { files } = await build(directory);
  const html = await readFile(resolve(directory, 'index.html'), 'utf8');
  const paths = new Set(files);
  for (const match of html.matchAll(/(?:src|href)="([^"#]+)"/g)) {
    const url = match[1];
    if (/^(?:https?:|data:)/.test(url)) continue;
    assert.ok(paths.has(url.split('?')[0].replace(/^\.\//, '')), `Missing HTML asset: ${url}`);
  }
  for (const name of files.filter(name => !name.startsWith('broglow/'))) {
    const source = await readFile(resolve(directory, name), 'utf8');
    assert.ok(!source.includes('../../'), `Workspace-relative path in ${name}`);
    for (const match of source.matchAll(/["'`](?:\.\/)?(broglow\/[\w./-]+\.(?:js|css|glb|svg|png|jpg))/g)) {
      assert.ok(paths.has(match[1]), `Missing source asset: ${match[1]} in ${name}`);
    }
  }
  for (const signal of ['green', 'yellow', 'red']) {
    assert.ok(paths.has(`broglow/site/assets/photos/signal-${signal}.jpg`));
  }
  for (const signal of ['green', 'yellow', 'red', 'none']) {
    assert.ok(paths.has(`broglow/assets/icons/broglow-${signal}.png`));
  }
  for (const asset of ['popover.css', 'popover.js', 'presentation.js', 'document.js']) {
    assert.ok(paths.has(`broglow/assets/popover/${asset}`));
  }
  assert.ok(paths.has('broglow/site/vendor/three-0.183.0/three.core.min.js'));
  assert.ok(files.every(name => !/\.test\.cjs$|\.md$|preview-|(?:^|\/)src\//.test(name)));
  assert.ok(!files.includes('comparison.html'));
});

test('packaged dynamic model import keeps an explicit relative module specifier', async t => {
  const directory = await fixture(t);
  await build(directory);
  const script = await readFile(resolve(directory, 'lamp.js'), 'utf8');
  assert.match(script, /import\(["']\.\/broglow\/site\/vendor\/model-viewer-module-4\.3\.1\.min\.js["']\)/);
  assert.doesNotMatch(script, /import\(["']broglow\//);
});

test('shared UI and vendor assets are copied byte-for-byte from the real app', async t => {
  const directory = await fixture(t);
  const { files } = await build(directory);
  for (const name of files.filter(name => name.startsWith('broglow/'))) {
    assert.deepEqual(await readFile(resolve(directory, name)),
      await readFile(resolve(__dirname, '../..', name.replace(/^broglow\//, ''))), name);
  }
});

test('build is reproducible and preserves files it does not own', async t => {
  const directory = await fixture(t);
  const { files } = await build(directory);
  const contents = await Promise.all([...files, '.broglow-build.json']
    .map(async name => [name, await readFile(resolve(directory, name))]));
  await writeFile(resolve(directory, 'keep.txt'), 'keep me');
  await build(directory);
  for (const [name, previous] of contents) {
    assert.deepEqual(await readFile(resolve(directory, name)), previous, name);
  }
  assert.equal(await readFile(resolve(directory, 'keep.txt'), 'utf8'), 'keep me');
});

test('build refuses an unrelated nonempty directory without changing it', async t => {
  const directory = await fixture(t);
  await writeFile(resolve(directory, 'index.html'), 'original');
  await assert.rejects(build(directory), /not empty/);
  assert.deepEqual(await readdir(directory), ['index.html']);
  assert.equal(await readFile(resolve(directory, 'index.html'), 'utf8'), 'original');
  await assert.rejects(build(__dirname), /dedicated build directory/);
  await assert.rejects(build(resolve(__dirname, '../..')), /dedicated build directory/);
});
