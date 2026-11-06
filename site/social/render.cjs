'use strict';

const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const { access, mkdir, mkdtemp, readFile, rm, writeFile } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { dirname, extname, join, resolve } = require('node:path');
const { pathToFileURL } = require('node:url');
const { render } = require('../../assets/popover/document.js');

const project = resolve(__dirname, '../..');
const delay = milliseconds => new Promise(resolveDelay => setTimeout(resolveDelay, milliseconds));
const help = `Render a BroGlow social-preview candidate from the approved layout and shipped app UI.

Usage: node site/social/render.cjs --output PATH.png [--browser PATH]

The output must be a new PNG file. Parent directories are created.
On macOS, the default browser is Google Chrome. Other platforms require --browser.
The approved typography uses macOS system fonts. Review candidates before publishing.
`;

function argumentsFor(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index++) {
    const name = argv[index];
    if (!['--output', '--browser'].includes(name) || !argv[index + 1]
      || argv[index + 1].startsWith('--')) {
      throw new Error(`Expected --output PATH.png and optional --browser PATH; received ${name}`);
    }
    const key = name.slice(2);
    if (options[key]) throw new Error(`Duplicate argument ${name}`);
    options[key] = argv[++index];
  }
  if (!options.output) throw new Error('Provide --output PATH.png. Use --help for usage.');
  options.output = resolve(options.output);
  if (extname(options.output).toLowerCase() !== '.png') throw new Error('Output must use the .png extension.');
  return options;
}

function sampleHost(model, now) {
  Date.now = () => now;
  window.ipc = { postMessage(raw) {
    if (JSON.parse(raw).cmd !== 'ready') return;
    window.__setState(model);
    document.fonts.ready.then(() => {
      const bounds = document.querySelector('.helprow').getBoundingClientRect();
      window.parent.postMessage({ type: 'broglow:sample-row', bounds: {
        left: bounds.left, top: bounds.top, width: bounds.width, height: bounds.height,
      } }, '*');
    });
  } };
}

async function cardPage() {
  const now = Date.parse('2026-11-06T12:00:00Z');
  const agents = {
    codex: { label: 'Codex', badge: 'cx', icon: 'M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5z' },
    claude: { label: 'Claude', badge: 'cc', icon: 'M8 1.5v13M1.5 8h13M3.4 3.4l9.2 9.2M12.6 3.4l-9.2 9.2' },
    opencode: { label: 'opencode', badge: 'oc', icon: 'M5.5 2.5H3v11h2.5M10.5 2.5H13v11h-2.5' },
  };
  const model = {
    aggregate: 'red',
    sessions: [
      { id: 'sample-codex', name: 'Approve the tests', project: 'website', status: 'needs_help',
        agent: agents.codex, host: { kind: 'app', name: 'Codex app' }, lastUpdated: new Date(now).toISOString() },
      { id: 'sample-claude', name: 'Build the homepage', project: 'website', status: 'active',
        agent: agents.claude, host: { kind: 'terminal', name: 'Terminal' }, lastUpdated: new Date(now).toISOString() },
      { id: 'sample-opencode', name: 'Review complete', project: 'broglow', status: 'inactive',
        agent: agents.opencode, host: { kind: 'terminal', name: 'Terminal' }, lastUpdated: new Date(now - 120000).toISOString() },
    ],
    settings: { showAgentNames: true, showDashboardButton: true, groupByProject: false,
      yellowMode: 'any_inactive', usageEnabled: false },
    lamp: { present: false, enabled: false, transport: 'usb', state: 'disabled', devices: [] },
    usage: null,
  };
  const popover = join(project, 'assets/popover');
  const text = path => readFile(path, 'utf8');
  const icons = {};
  for (const color of ['red', 'yellow', 'green', 'none']) {
    icons[color] = 'data:image/png;base64,'
      + (await readFile(join(project, `assets/icons/broglow-${color}.png`))).toString('base64');
  }
  const app = render({
    template: await text(join(project, 'assets/popover.html')),
    styles: await text(join(popover, 'popover.css')),
    script: await text(join(popover, 'popover.js')),
    presentation: await text(join(popover, 'presentation.js')),
    hostScript: `(${sampleHost.toString()})(${JSON.stringify(model)}, ${now});`,
    platform: 'mac',
    icons,
  });
  let card = await text(join(__dirname, 'card.html'));
  const marker = '__BROGLOW_APP_DOCUMENT__';
  assert.equal(card.split(marker).length, 2, 'Card must contain one app-document slot.');
  card = card.replace(marker, () => JSON.stringify(app).replaceAll('<', '\\u003c'));
  for (const [source, path, mime] of [
    ['buddy.png', join(__dirname, 'buddy.png'), 'image/png'],
    ['../guide/assets/favicon.svg', join(project, 'site/guide/assets/favicon.svg'), 'image/svg+xml'],
  ]) {
    const bytes = await readFile(path);
    card = card.replace(`src="${source}"`, () => `src="data:${mime};base64,${bytes.toString('base64')}"`);
  }
  return card;
}

async function connect(url) {
  const socket = new WebSocket(url);
  await new Promise((resolveOpen, reject) => {
    socket.addEventListener('open', resolveOpen, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener('message', event => {
    const message = JSON.parse(event.data);
    const request = pending.get(message.id);
    if (!request) return;
    pending.delete(message.id);
    clearTimeout(request.timer);
    if (message.error) request.reject(new Error(message.error.message));
    else request.resolve(message.result);
  });
  return {
    call(method, params = {}) {
      return new Promise((resolveCall, reject) => {
        const id = ++sequence;
        const timer = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`Browser command timed out: ${method}`));
        }, 10000);
        pending.set(id, { resolve: resolveCall, reject, timer });
        socket.send(JSON.stringify({ id, method, params }));
      });
    },
    close() { socket.close(); },
  };
}

async function screenshot(options) {
  try {
    await access(options.output);
    throw new Error(`Output already exists: ${options.output}. Choose a new candidate filename.`);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const browser = options.browser || (process.platform === 'darwin'
    ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : null);
  if (!browser) throw new Error('Provide --browser PATH on this platform.');
  await access(browser);
  const temporary = await mkdtemp(join(tmpdir(), 'broglow-social-'));
  let chrome;
  let client;
  let browserError;
  try {
    await writeFile(join(temporary, 'card.html'), await cardPage());
    const profile = join(temporary, 'profile');
    chrome = spawn(browser, [
      '--headless=new', '--disable-gpu', '--hide-scrollbars', '--no-first-run',
      '--no-default-browser-check', '--disable-background-networking', '--disable-extensions',
      '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank',
    ], { stdio: 'ignore' });
    chrome.on('error', error => { browserError = error; });
    let port;
    for (let attempt = 0; attempt < 100; attempt++) {
      if (browserError) throw browserError;
      if (chrome.exitCode !== null) throw new Error(`Browser exited with code ${chrome.exitCode}.`);
      try {
        port = (await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0];
        break;
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
        await delay(100);
      }
    }
    if (!port) throw new Error('Browser did not start within 10 seconds.');
    const response = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(10000) });
    if (!response.ok) throw new Error(`Browser discovery failed with HTTP ${response.status}.`);
    const page = (await response.json()).find(item => item.type === 'page');
    if (!page) throw new Error('Browser did not expose a page.');
    client = await connect(page.webSocketDebuggerUrl);
    const evaluate = async expression => {
      const result = await client.call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
      return result.result.value;
    };
    await client.call('Page.enable');
    await client.call('Emulation.setDeviceMetricsOverride', {
      width: 1200, height: 630, deviceScaleFactor: 1, mobile: false,
    });
    await client.call('Page.navigate', { url: pathToFileURL(join(temporary, 'card.html')).href });
    let ready = false;
    for (let attempt = 0; attempt < 100; attempt++) {
      ready = await evaluate(`document.querySelector('.app-detail')?.dataset.ready === 'true'
        && document.fonts.status === 'loaded'
        && Array.from(document.images).every(image => image.complete && image.naturalWidth > 0)`);
      if (ready) break;
      await delay(100);
    }
    if (!ready) throw new Error('Images, fonts or app detail did not become ready within 10 seconds.');
    await evaluate('Promise.all(Array.from(document.images, image => image.decode())).then(() => true)');
    const fits = await evaluate(`Array.from(document.querySelectorAll('.brand, .category, h1, .subtitle, .attention, .buddy, .app-detail, .footer')).every(element => {
      const bounds = element.getBoundingClientRect();
      return bounds.left >= 0 && bounds.top >= 0 && bounds.right <= 1200 && bounds.bottom <= 630;
    })`);
    if (!fits) throw new Error('Preview content extends beyond the 1200 x 630 canvas.');
    const image = await client.call('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    const bytes = Buffer.from(image.data, 'base64');
    assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
    assert.equal(bytes.readUInt32BE(16), 1200);
    assert.equal(bytes.readUInt32BE(20), 630);
    await mkdir(dirname(options.output), { recursive: true });
    await writeFile(options.output, bytes, { flag: 'wx' });
    console.log(`Rendered ${options.output} (1200 x 630, ${bytes.length} bytes). Review the candidate before publishing.`);
  } finally {
    client?.close();
    if (chrome && chrome.exitCode === null) {
      const exited = new Promise(resolveExit => chrome.once('exit', resolveExit));
      chrome.kill('SIGTERM');
      await Promise.race([exited, delay(2000)]);
      if (chrome.exitCode === null) chrome.kill('SIGKILL');
    }
    await rm(temporary, { recursive: true, force: true });
  }
}

async function main() {
  if (process.argv.length === 3 && process.argv[2] === '--help') {
    console.log(help);
    return;
  }
  await screenshot(argumentsFor(process.argv.slice(2)));
}

main().catch(error => {
  console.error(error.message);
  process.exitCode = 1;
});
