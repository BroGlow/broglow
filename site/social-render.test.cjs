const assert = require('node:assert/strict');
const {spawnSync} = require('node:child_process');
const {mkdtempSync, readFileSync, rmSync, writeFileSync} = require('node:fs');
const {tmpdir} = require('node:os');
const {join, resolve} = require('node:path');
const test = require('node:test');

const project = resolve(__dirname, '..');
const renderer = resolve(__dirname, 'social/render.cjs');

test('social renderer help succeeds without a browser', () => {
  const result = spawnSync(process.execPath, [renderer, '--help'], {
    cwd: project, encoding: 'utf8', timeout: 5000,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, '');
  assert.match(result.stdout, /Usage: node site\/social\/render\.cjs --output PATH\.png/);
});

test('social renderer requires an explicit output path', () => {
  const result = spawnSync(process.execPath, [renderer], {
    cwd: project, encoding: 'utf8', timeout: 5000,
  });
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /Provide --output PATH\.png/);
});

test('social renderer rejects unknown and duplicate arguments', () => {
  for (const [args, message] of [
    [['--unknown', 'value'], /received --unknown/],
    [['--output', 'first.png', '--output', 'second.png'], /Duplicate argument --output/],
  ]) {
    const result = spawnSync(process.execPath, [renderer, ...args], {
      cwd: project, encoding: 'utf8', timeout: 5000,
    });
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr, message);
  }
});

test('social renderer rejects output formats other than PNG', () => {
  const result = spawnSync(process.execPath, [renderer, '--output', 'candidate.jpg'], {
    cwd: project, encoding: 'utf8', timeout: 5000,
  });
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /Output must use the \.png extension/);
});

test('social renderer preserves existing output before accessing a browser', t => {
  const directory = mkdtempSync(join(tmpdir(), 'broglow-social-cli-'));
  t.after(() => rmSync(directory, {recursive: true, force: true}));
  const output = join(directory, 'approved.png');
  const original = readFileSync(resolve(__dirname, 'assets/social/broglow-og-v1.png'));
  writeFileSync(output, original);

  const result = spawnSync(process.execPath, [
    renderer, '--output', output, '--browser', join(directory, 'missing-browser'),
  ], {cwd: project, encoding: 'utf8', timeout: 5000});

  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /Output already exists/);
  assert.deepEqual(readFileSync(output), original);
});
