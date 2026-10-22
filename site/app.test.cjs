const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { resolve } = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = readFileSync(resolve(__dirname, 'app.js'), 'utf8');
class DetailEvent extends Event {
  constructor(type, detail) { super(type); this.detail = detail; }
}
function preview(now = new Date()) {
  const nodes = new Map();
  let clockTime = now.getTime();
  const intervals = [];
  const clockFormats = [];
  class ClockDate extends Date {
    constructor() { super(clockTime); }
    toLocaleTimeString(locale, options) {
      clockFormats.push([locale, {...options}]);
      return super.toLocaleTimeString(locale, options);
    }
  }
  function element(dataset = {}) {
    const node = new EventTarget();
    Object.assign(node, {dataset, attributes:{}, style:{}, hidden:false, textContent:'', innerHTML:''});
    const classes = new Set();
    node.classList = {
      add: name => classes.add(name), remove: name => classes.delete(name), contains: name => classes.has(name),
      toggle(name, force) {
        const enabled = force ?? !classes.has(name);
        enabled ? classes.add(name) : classes.delete(name);
        return enabled;
      },
    };
    node.setAttribute = (name, value) => { node.attributes[name] = value; };
    node.focus = () => { nodes.set('focused', node); };
    node.select = () => { node.selected = true; };
    node.append = child => { nodes.set(`#${child.id}`, child); };
    return node;
  }
  const $ = selector => {
    if (!nodes.has(selector)) nodes.set(selector, element());
    return nodes.get(selector);
  };
  const buttons = ['green','yellow','red'].map(signal => element({heroState:signal}));
  const photos = ['green','yellow','red'].map(signal => element({signalChoice:signal}));
  const browser = new EventTarget();
  browser.BroGlowPresentation = require('../assets/popover/presentation.js');
  const body = element();
  const stateEvents = [];
  browser.addEventListener('broglow:app-state', event => stateEvents.push(event.detail.signal));
  vm.runInNewContext(source, {
    document: {body, querySelector:selector => selector === '#install-fallback' ? nodes.get(selector) : $(selector),
      querySelectorAll:selector => selector === '[data-hero-state]' ? buttons : photos, createElement:() => element()},
    window:browser,
    navigator:{clipboard:{writeText:async value => { nodes.set('clipboard', value); }}},
    CustomEvent:class extends DetailEvent { constructor(type, options) { super(type,options.detail); } },
    clearTimeout:() => {}, setTimeout:() => 1,
    Date:ClockDate,
    setInterval:(callback,delay) => { intervals.push({callback,delay}); return intervals.length; },
  });
  return { $, body, browser, stateEvents, nodes,
    clock:{intervals,formats:clockFormats,setTime:date => { clockTime = date.getTime(); }},
    click:selector => $(selector).dispatchEvent(new Event('click')),
    state:signal => buttons.find(button => button.dataset.heroState === signal).dispatchEvent(new Event('click')),
    photo:signal => photos.find(button => button.dataset.signalChoice === signal).dispatchEvent(new Event('click')),
    send:(name,detail) => browser.dispatchEvent(new DetailEvent(name,detail)),
    language(dictionary) {
      browser.BroGlowI18n = {t:(key,vars = {}) => (dictionary[key] || key).replace(/\{(\w+)\}/g,(_,name) => vars[name])};
      browser.dispatchEvent(new DetailEvent('broglow:language',{language:'ru'}));
    },
  };
}

test('workspace clock uses the visitor locale and refreshes across midnight and on focus', () => {
  const beforeMidnight = new Date(2026, 9, 22, 23, 59, 59);
  const demo = preview(beforeMidnight);
  const clock = demo.$('#workspace-clock');
  const options = {hour:'2-digit',minute:'2-digit'};
  assert.equal(clock.textContent, beforeMidnight.toLocaleTimeString(undefined, options));
  assert.equal(clock.dateTime, beforeMidnight.toISOString());
  assert.deepEqual(demo.clock.formats[0], [undefined, options], 'device locale and timezone remain the defaults');
  assert.equal(demo.clock.intervals.length, 1);
  assert.equal(demo.clock.intervals[0].delay, 1000);

  const afterMidnight = new Date(beforeMidnight.getTime() + 1000);
  demo.clock.setTime(afterMidnight);
  demo.clock.intervals[0].callback();
  assert.equal(clock.textContent, afterMidnight.toLocaleTimeString(undefined, options));
  assert.equal(clock.dateTime, afterMidnight.toISOString());

  const afterSleep = new Date(afterMidnight.getTime() + 90 * 60 * 1000);
  demo.clock.setTime(afterSleep);
  demo.browser.dispatchEvent(new Event('focus'));
  assert.equal(clock.textContent, afterSleep.toLocaleTimeString(undefined, options));
  assert.equal(clock.dateTime, afterSleep.toISOString());
});

test('all three hero states tell a complete checkout story and use the shipped tray icons', () => {
  const demo = preview();
  for (const [signal, expected, cursor, question] of [
    ['green','Running checkout tests…',true,false],
    ['yellow','12 tests passed',false,false],
    ['red','Checkout tests are ready to run',false,true],
  ]) {
    demo.state(signal);
    assert.equal(demo.body.dataset.signal,signal);
    assert.equal(demo.$('#terminal-title').textContent,'~/shopfront');
    assert.equal(demo.$('#terminal-prompt').textContent,'❯ Test the checkout flow.');
    assert.ok(demo.$('#terminal-output').innerHTML.includes(expected));
    assert.equal(demo.$('#terminal-output').innerHTML.includes('terminal-cursor'),cursor);
    assert.equal(demo.$('#demo-question').hidden,!question);
    assert.equal(demo.$('#tray-icon').src,`../assets/icons/broglow-${signal}.png`);
  }
  assert.equal(demo.$('#demo-question p').textContent,'Can I run the checkout tests?');
  assert.deepEqual(demo.stateEvents,['green','yellow','red']);
});

test('opening the waiting app session focuses its permission, and approval resumes the same task', () => {
  const demo = preview();
  demo.state('red');
  demo.send('broglow:open-session',{session:'codex'});
  assert.equal(demo.nodes.get('focused'),demo.$('#approve-demo'));
  assert.ok(demo.$('#terminal').classList.contains('focused'));
  assert.equal(demo.$('#terminal-title').textContent,'~/shopfront');
  demo.click('#approve-demo');
  assert.equal(demo.body.dataset.signal,'green');
  assert.equal(demo.$('#demo-question').hidden,true);
  assert.ok(demo.$('#terminal-output').innerHTML.includes('Permission granted in demo'));
  assert.ok(demo.$('#terminal-output').innerHTML.includes('Running checkout tests…'));
  assert.equal(demo.nodes.get('focused'),demo.$('#terminal'));
  assert.equal(demo.stateEvents.at(-1),'green');
  demo.send('broglow:app-ready');
  assert.ok(demo.$('#terminal-output').innerHTML.includes('Permission granted in demo'),'late app initialization preserves approval');
});

test('pointer and keyboard activation raise either window without resetting the demo', () => {
  const demo = preview();
  demo.state('red');
  demo.send('broglow:app-activate');
  demo.send('broglow:open-session', { session: 'codex' });
  assert.ok(demo.$('#terminal').classList.contains('focused'), 'opening a session wins after its app activation');
  const output = demo.$('#terminal-output').innerHTML;
  demo.send('broglow:app-activate');
  assert.equal(demo.$('#terminal').classList.contains('focused'), false, 'clicking the iframe raises BroGlow');
  for (const type of ['pointerdown', 'focusin']) {
    demo.$('#terminal').dispatchEvent(new Event(type));
    assert.ok(demo.$('#terminal').classList.contains('focused'));
    demo.$('#session-panel').dispatchEvent(new Event(type));
    assert.equal(demo.$('#terminal').classList.contains('focused'), false);
  }
  assert.equal(demo.$('#terminal-output').innerHTML, output);
  assert.equal(demo.$('#demo-question').hidden, false, 'the waiting permission survives window switching');
  assert.deepEqual(demo.stateEvents, ['red']);
});

test('tray close, reopen and app hide report panel visibility without resetting sessions', () => {
  const demo = preview();
  const visibility = [];
  demo.browser.addEventListener('broglow:app-visibility', event => visibility.push(event.detail.open));
  demo.state('red');
  demo.click('#tray-toggle');
  assert.equal(demo.$('#session-panel').inert, true);
  demo.click('#tray-toggle');
  assert.equal(demo.$('#session-panel').inert, false);
  demo.send('broglow:app-action', {action:'hide'});
  assert.equal(demo.$('#session-panel').inert, true);
  assert.deepEqual(visibility, [false, true, false]);
  assert.deepEqual(demo.stateEvents, ['red']);
});

test('language changes retain approval, focus, photo and model states, and copied commands', async () => {
  const demo = preview();
  demo.state('red');
  demo.send('broglow:open-session',{session:'codex'});
  demo.click('#approve-demo');
  demo.photo('yellow');
  demo.send('broglow:model-signal',{signal:'red',brightness:0.4});
  demo.click('#copy-install');
  await new Promise(resolve => setImmediate(resolve));
  demo.language({
    'state.working':'Работает','state.paused':'Пауза','state.needsYou':'Нужна помощь',
    'terminal.permissionGranted':'Разрешение получено','terminal.runningTests':'Запускаю тесты…',
    'tray.alt':'BroGlow: {state}','photo.alt':'Лампа: {state}',
    'model.status':'{state} · яркость {brightness}%','copy.done':'Скопировано ✓',
    'toast.resumed':'Работа продолжается.',
  });
  assert.equal(demo.body.dataset.signal,'green');
  assert.ok(demo.$('#terminal-output').innerHTML.includes('Разрешение получено'));
  assert.ok(demo.$('#terminal-output').innerHTML.includes('Запускаю тесты…'));
  assert.equal(demo.$('#demo-question').hidden,true);
  assert.ok(demo.$('#terminal').classList.contains('focused'));
  assert.equal(demo.$('#tray-icon').alt,'BroGlow: Работает');
  assert.equal(demo.$('#signal-photo').alt,'Лампа: Пауза');
  assert.ok(demo.$('#signal-photo').src.endsWith('signal-yellow.jpg'));
  assert.equal(demo.$('#model-status').textContent,'Нужна помощь · яркость 40%');
  assert.ok(demo.$('#lamp-fallback').src.endsWith('signal-red.jpg'));
  assert.equal(demo.$('#copy-install').textContent,'Скопировано ✓');
  assert.equal(demo.$('#toast').textContent,'Работа продолжается.');
  assert.equal(demo.stateEvents.length,2,'translation does not reset the actual app state');
});

test('language changes keep a waiting permission visible; other selected app sessions keep their own prompt', () => {
  const demo = preview();
  demo.state('red');
  demo.language({'terminal.permissionQuestion':'Запустить тесты?','terminal.allowDemo':'Разрешить'});
  assert.equal(demo.body.dataset.signal,'red');
  assert.equal(demo.$('#demo-question').hidden,false);
  assert.equal(demo.$('#demo-question p').textContent,'Запустить тесты?');
  assert.ok(demo.$('#approve-demo').innerHTML.includes('Разрешить'));
  demo.send('broglow:open-session',{session:'opencode'});
  assert.equal(demo.$('#terminal-title').textContent,'~/broglow');
  assert.equal(demo.$('#terminal-prompt').textContent,'❯ Polish the little details.');
  assert.equal(demo.$('#demo-question').hidden,true);
  demo.language({'terminal.promptDetails':'❯ Исправить детали.'});
  assert.equal(demo.$('#terminal-prompt').textContent,'❯ Исправить детали.');
  demo.send('broglow:open-session',{session:'foreign'});
  assert.equal(demo.$('#terminal-title').textContent,'~/broglow');
  demo.click('#approve-demo');
  assert.equal(demo.body.dataset.signal,'red','a hidden approval cannot resume a different session');
});

test('native aggregate updates change only the tray, preserving the scenario and dismissed sessions', () => {
  const demo = preview();
  demo.state('yellow');
  demo.send('broglow:app-aggregate', { signal: 'green' });
  assert.equal(demo.body.dataset.signal, 'green');
  assert.ok(demo.$('#tray-icon').src.endsWith('broglow-green.png'));
  assert.equal(demo.$('#workspace-hint').textContent, 'Actual BroGlow interface. Illustrative local sessions.');
  assert.ok(demo.$('#terminal-output').innerHTML.includes('12 tests passed'));
  demo.send('broglow:app-aggregate', { signal: 'none' });
  assert.ok(demo.$('#tray-icon').src.endsWith('broglow-none.png'));
  assert.equal(demo.$('#tray-icon').alt, 'BroGlow: No live sessions');
  assert.equal(demo.$('#workspace-hint').textContent, 'No live sessions. Choose a session state to try again.');
  demo.language({ 'state.noSessions': 'Нет активных сессий' });
  assert.equal(demo.$('#tray-icon').alt, 'BroGlow: Нет активных сессий');
  assert.deepEqual(demo.stateEvents, ['yellow'], 'aggregate and language updates never reset sessions');
  demo.send('broglow:app-aggregate', { signal: 'arbitrary' });
  assert.equal(demo.body.dataset.signal, 'none');
});

test('photo fallback disables brightness and restores the 3D controls and labels on recovery', () => {
  const demo = preview();
  demo.send('broglow:model-error');
  assert.equal(demo.$('#lamp-brightness').disabled, true);
  assert.equal(demo.$('#lamp-fallback').hidden, false);
  assert.equal(demo.$('#model-status').textContent, 'Working · photo preview');
  demo.send('broglow:model-signal', { signal: 'red', brightness: 0.4 });
  assert.equal(demo.$('#model-status').textContent, 'Needs you · photo preview');
  assert.ok(demo.$('#lamp-fallback').src.endsWith('signal-red.jpg'));
  demo.language({ 'state.needsYou': 'Нужен ответ', 'model.photoStatus': '{state} · фото',
    'model.photoCaption': 'НАША ЛАМПА · ФОТО' });
  assert.equal(demo.$('#model-status').textContent, 'Нужен ответ · фото');
  assert.equal(demo.$('#model-caption').textContent, 'НАША ЛАМПА · ФОТО');
  demo.send('broglow:model-ready');
  assert.equal(demo.$('#lamp-brightness').disabled, false);
  assert.equal(demo.$('#lamp-fallback').hidden, true);
  assert.equal(demo.$('#model-caption').textContent, 'OUR LAMP · 3D RENDER · DRAG TO EXPLORE');
  assert.equal(demo.$('#model-status').textContent, 'Нужен ответ · 40% brightness');
});


test('menu-bar usage displays the shared readout and clears when hidden', () => {
  const demo = preview();
  demo.send('broglow:app-usage', {usage:{todayTokens:1312400, agents:[
    {label:'Claude',badge:'cc',icon:'M8 1v14',limits:[{usedPercent:62}]},
    {label:'Codex',badge:'cx',limits:[{usedPercent:12}]},
    {label:'opencode',badge:'oc',limits:[]},
  ]}});
  const readout = demo.$('#tray-usage');
  assert.equal(readout.hidden, false);
  assert.equal(readout.attributes['aria-label'], 'Claude 62% · Codex 12%');
  assert.match(readout.innerHTML, /<svg/);
  assert.doesNotMatch(readout.innerHTML, /opencode/);
  demo.send('broglow:app-usage', {usage:null});
  assert.equal(readout.hidden, true);
  assert.equal(readout.innerHTML, '');
});
