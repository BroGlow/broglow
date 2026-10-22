import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import test from 'node:test';
import { runInNewContext } from 'node:vm';

const require = createRequire(import.meta.url);
const { render } = require('../../assets/popover/document.js');
const { agentMark, tokens, menuBarUsage } = require('../../assets/popover/presentation.js');
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const assets = {
  template: read('../../assets/popover.html'),
  styles: read('../../assets/popover/popover.css'),
  script: read('../../assets/popover/popover.js'),
  presentation: read('../../assets/popover/presentation.js'),
  icons: Object.fromEntries(['red', 'yellow', 'green', 'none'].map(color => [color,
    `data:image/png;base64,${readFileSync(new URL(`../../assets/icons/broglow-${color}.png`, import.meta.url)).toString('base64')}`])),
};

test('native and demo documents use identical view sources and an explicit host slot', () => {
  const native = render(assets);
  const browserGlobal = {};
  runInNewContext(read('../../assets/popover/document.js'), browserGlobal);
  assert.equal(browserGlobal.BroGlowPopoverDocument.render(assets), native);
  const host = 'window.ipc = {postMessage() {}};';
  const demo = render({ ...assets, hostScript: host });
  assert.equal(demo.replace(`<script>${host}</script>`, ''), native);
  assert.ok(demo.indexOf(host) < demo.indexOf('window.__setState'));
  assert.ok(demo.includes(assets.styles));
  assert.doesNotMatch(demo, /__[A-Z][A-Z0-9_]*__/);
  assert.match(render({ ...assets, platform: 'win' }), /body class="win"/);
});

test('document assembly fails clearly on missing assets, slots and unknown template tokens', () => {
  for (const key of ['template', 'styles', 'script', 'presentation']) {
    assert.throws(() => render({ ...assets, [key]: '' }), /Missing popover/);
  }
  for (const token of ['__POPOVER_STYLE__', '__POPOVER_SCRIPT__', '__POPOVER_HOST__', '__POPOVER_PRESENTATION__']) {
    assert.throws(() => render({ ...assets, template: assets.template.replace(token, '') }), /Expected one popover slot/);
    assert.throws(() => render({ ...assets, template: assets.template + token }), /Expected one popover slot/);
  }
  assert.throws(() => render({ ...assets, icons: {} }), /invalid popover/);
  assert.throws(() => render({ ...assets, platform: 'other' }), /Invalid popover platform/);
  assert.throws(() => render({ ...assets, template: assets.template + '__NEW_REQUIRED_ASSET__' }), /Unresolved/);
});

test('shared usage readout uses each provider first limit and never combines percentages', () => {
  const readout = menuBarUsage({ todayTokens: 1312400, agents: [
    { label: 'Claude', badge: 'cc', icon: 'M8 1v14', limits: [{ usedPercent: 62 }, { usedPercent: 31 }] },
    { label: 'Codex', badge: 'cx', limits: [{ usedPercent: 12.4 }] },
    { label: 'opencode', badge: 'oc', limits: [] },
  ] });
  assert.equal(readout.label, 'Claude 62% · Codex 12%');
  assert.match(readout.html, /<svg/);
  assert.doesNotMatch(readout.html, /31%|74%|opencode/);
  assert.equal(menuBarUsage({ todayTokens: 12400, agents: [] }).label, '12k tok');
  assert.equal(menuBarUsage({ todayTokens: 0, agents: [] }).label, '');
  assert.equal(menuBarUsage(null).html, '');
  assert.equal(tokens(1312400), '1.3M');
});

test('shared agent marks escape registry text and SVG path attributes', () => {
  const mark = agentMark({ label: '<script>', icon: '" onload="bad' }, false);
  assert.match(mark, /aria-label="&lt;script&gt;"/);
  assert.match(mark, /d="&quot; onload=&quot;bad"/);
  assert.doesNotMatch(mark, /<script>| d="" onload=/);
  assert.match(agentMark({ label: 'Future agent', badge: '<x>' }, false), /&lt;x&gt;/);
});
