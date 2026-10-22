(() => {
  const $ = selector => document.querySelector(selector);
  const $$ = selector => [...document.querySelectorAll(selector)];
  function renderClock() {
    const now = new Date();
    const clock = $('#workspace-clock');
    clock.textContent = now.toLocaleTimeString(undefined, {hour:'2-digit', minute:'2-digit'});
    clock.dateTime = now.toISOString();
  }
  renderClock();
  setInterval(renderClock, 1000);
  window.addEventListener('focus', renderClock);
  const photos = {green:'signal-green.jpg',yellow:'signal-yellow.jpg',red:'signal-red.jpg'};
  const stateKeys = {green:['state.working','Working'],yellow:['state.paused','Paused'],red:['state.needsYou','Needs you'],none:['state.noSessions','No live sessions']};
  const text = (key, fallback, vars = {}) => {
    const translated = window.BroGlowI18n?.t(key, vars);
    return (translated && translated !== key ? translated : fallback).replace(/\{(\w+)\}/g, (_, name) => vars[name] ?? `{${name}}`);
  };
  const label = signal => text(...stateKeys[signal]);
  const escape = value => value.replace(/[&<>"']/g, character => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[character]));
  const line = (key, fallback, working = false) => `<p${working ? ' class="working-line"' : ''}>${working ? '' : '<span class="check">✓</span> '}${escape(text(key, fallback))}${working ? '<span class="terminal-cursor"></span>' : ''}</p>`;
  let heroSignal = 'green';
  let aggregateSignal = 'green';
  let session = 'codex';
  let approved = false;
  let photoSignal = 'green';
  let modelSignal = 'green';
  let modelBrightness = 75;
  let modelFailed = false;
  let copyState = '';
  let toast;
  let toastTimer;
  function announce(key, fallback) {
    toast = [key, fallback];
    clearTimeout(toastTimer);
    $('#toast').textContent = text(...toast);
    $('#toast').hidden = false;
    toastTimer = setTimeout(() => { $('#toast').hidden = true; }, 3500);
  }
  function renderTerminal() {
    const checkout = session === 'codex';
    const signal = checkout ? heroSignal : 'green';
    $('#terminal-title').textContent = checkout ? '~/shopfront' : session === 'opencode' ? '~/broglow' : '~/good-things';
    $('#terminal-prompt').textContent = checkout ? text('terminal.promptCheckout','❯ Test the checkout flow.') : session === 'opencode' ? text('terminal.promptDetails','❯ Polish the little details.') : text('terminal.promptHomepage','❯ Build the new homepage.');
    $('#demo-question').hidden = signal !== 'red';
    $('#demo-question p').textContent = text('terminal.permissionQuestion','Can I run the checkout tests?');
    $('#approve-demo').innerHTML = `${escape(text('terminal.allowDemo','Allow in demo'))} <span aria-hidden="true">↵</span>`;
    if (!checkout) {
      $('#terminal-output').innerHTML = line('terminal.projectRead','Read project files') + line('terminal.changesWorking','Working on your changes…', true);
    } else if (signal === 'yellow') {
      $('#terminal-output').innerHTML = line('terminal.testsAdded','Added checkout tests') + line('terminal.testsPassed','12 tests passed') + `<p>${escape(text('terminal.turnComplete','Checkout flow verified. Ready for your next prompt.'))}</p>`;
    } else if (signal === 'red') {
      $('#terminal-output').innerHTML = line('terminal.filesRead','Read checkout files') + line('terminal.testsAdded','Added checkout tests') + `<p>${escape(text('terminal.permissionReady','Checkout tests are ready to run'))}</p>`;
    } else {
      $('#terminal-output').innerHTML = (approved ? line('terminal.permissionGranted','Permission granted in demo') : line('terminal.filesRead','Read checkout files')) + line('terminal.testsAdded','Added checkout tests') + line('terminal.runningTests','Running checkout tests…', true);
    }
    $('#terminal-status').textContent = signal === 'red' ? text('terminal.waiting','Waiting for your permission') : signal === 'yellow' ? text('terminal.paused','Turn complete · ready for your next prompt') : text('terminal.working','Working locally');
  }
  function renderHero() {
    document.body.dataset.signal = aggregateSignal;
    $$('[data-hero-state]').forEach(button => button.setAttribute('aria-pressed',String(button.dataset.heroState === heroSignal)));
    const icon = $('#tray-icon');
    if (icon) {
      icon.src = `../../assets/icons/broglow-${aggregateSignal}.png`;
      icon.alt = text('tray.alt','BroGlow: {state}',{state:label(aggregateSignal)});
    }
    $('#workspace-hint').textContent = aggregateSignal === 'red' ? text('workspace.red','A session needs input. Click it in the BroGlow panel.') : aggregateSignal === 'yellow' ? text('workspace.yellow','A turn is done. Ready when you are.') : aggregateSignal === 'none' ? text('workspace.none','No live sessions. Choose a session state to try again.') : text('workspace.green','Actual BroGlow interface. Illustrative local sessions.');
    renderTerminal();
  }
  function setHeroSignal(signal) {
    if (!photos[signal]) return;
    heroSignal = signal;
    aggregateSignal = signal;
    session = 'codex';
    approved = false;
    $('#terminal').classList.remove('focused');
    renderHero();
    window.dispatchEvent(new CustomEvent('broglow:app-state',{detail:{signal}}));
  }
  $$('[data-hero-state]').forEach(button => button.addEventListener('click',() => setHeroSignal(button.dataset.heroState)));
  window.addEventListener('broglow:app-ready',() => {
    renderHero();
    window.dispatchEvent(new CustomEvent('broglow:app-state',{detail:{signal:heroSignal}}));
  });
  window.addEventListener('broglow:app-aggregate',event => {
    if (!stateKeys[event.detail?.signal]) return;
    aggregateSignal = event.detail.signal;
    renderHero();
  });
  window.addEventListener('broglow:app-usage', event => {
    const readout = window.BroGlowPresentation.menuBarUsage(event.detail?.usage);
    const element = $('#tray-usage');
    element.innerHTML = readout.html;
    element.hidden = !readout.label;
    element.setAttribute('aria-label', readout.label);
    element.title = readout.label;
  });
  for (const type of ['pointerdown', 'focusin']) {
    $('#terminal').addEventListener(type, () => $('#terminal').classList.add('focused'));
    $('#session-panel').addEventListener(type, () => $('#terminal').classList.remove('focused'));
  }
  window.addEventListener('broglow:app-activate', () => $('#terminal').classList.remove('focused'));
  window.addEventListener('broglow:app-action',event => {
    if (event.detail?.action === 'quit' || event.detail?.action === 'hide') {
      $('#session-panel').classList.add('collapsed');
      $('#session-panel').inert = true;
      $('#tray-toggle').setAttribute('aria-expanded','false');
      window.dispatchEvent(new CustomEvent('broglow:app-visibility',{detail:{open:false}}));
      $('#tray-toggle').focus({preventScroll:true});
      announce('toast.panelClosed','Demo panel closed. Use the menu-bar light to open it again.');
    } else announce('toast.dashboard','The real app opens its terminal dashboard here. This preview stays in your browser.');
  });
  window.addEventListener('broglow:app-error',() => {$('#app-preview-error').hidden = false;$('#app-preview').hidden = true;});
  window.addEventListener('broglow:app-resize',event => {
    const height = event.detail?.height;
    if (Number.isFinite(height)) $('#desktop').style.minHeight = `${Math.max(365,height + 80)}px`;
  });
  window.addEventListener('broglow:open-session',event => {
    if (!['codex','claude','opencode'].includes(event.detail?.session)) return;
    session = event.detail.session;
    const waiting = heroSignal === 'red' && session === 'codex';
    renderTerminal();
    $('#terminal').classList.add('focused');
    (waiting ? $('#approve-demo') : $('#terminal')).focus({preventScroll:true});
    announce(waiting ? 'toast.waiting' : 'toast.session',waiting ? 'Returned to the waiting session. Allow the simulated test run.' : 'Returned to this illustrative local session.');
  });
  $('#approve-demo').addEventListener('click',() => {
    if (heroSignal !== 'red' || session !== 'codex') return;
    heroSignal = 'green';
    aggregateSignal = 'green';
    approved = true;
    renderHero();
    window.dispatchEvent(new CustomEvent('broglow:app-state',{detail:{signal:heroSignal}}));
    $('#terminal').classList.add('focused');
    $('#terminal').focus({preventScroll:true});
    announce('toast.resumed','Back in flow. No commands were run.');
  });
  $('#tray-toggle').addEventListener('click',() => {
    const closed = $('#session-panel').classList.toggle('collapsed');
    $('#session-panel').inert = closed;
    $('#tray-toggle').setAttribute('aria-expanded',String(!closed));
    window.dispatchEvent(new CustomEvent('broglow:app-visibility',{detail:{open:!closed}}));
    $('#terminal').classList.remove('focused');
  });
  function renderPhoto() {
    $('#signal-photo').src = `../../site/assets/photos/${photos[photoSignal]}`;
    $('#signal-photo').alt = text('photo.alt','Real BroGlow lamp showing the {state} signal',{state:label(photoSignal)});
    $('#photo-state').innerHTML = `<i class="signal-dot ${photoSignal}"></i> ${escape(label(photoSignal))}`;
  }
  $$('[data-signal-choice]').forEach(button => button.addEventListener('click',() => {
    if (!photos[button.dataset.signalChoice]) return;
    photoSignal = button.dataset.signalChoice;
    $$('[data-signal-choice]').forEach(other => {other.classList.toggle('selected',other === button);other.setAttribute('aria-pressed',String(other === button));});
    renderPhoto();
  }));
  function renderModel() {
    $('#lamp-fallback').src = `../../site/assets/photos/${photos[modelSignal]}`;
    $('#lamp-fallback').alt = text('model.photoAlt','Our real BroGlow lamp showing the {state} signal',{state:label(modelSignal)});
    $('#lamp-brightness').disabled = modelFailed;
    $('#model-status').textContent = modelFailed
      ? text('model.photoStatus','{state} · photo preview',{state:label(modelSignal)})
      : text('model.status','{state} · {brightness}% brightness',{state:label(modelSignal),brightness:modelBrightness});
    $('#model-caption').textContent = modelFailed ? text('model.photoCaption','OUR REAL LAMP · PHOTO PREVIEW') : text('model.caption','OUR LAMP · 3D RENDER · DRAG TO EXPLORE');
  }
  window.addEventListener('broglow:model-signal',event => {
    const {signal,brightness} = event.detail;
    if (!photos[signal] || !Number.isFinite(brightness)) return;
    modelSignal = signal;
    modelBrightness = Math.round(brightness * 100);
    renderModel();
  });
  window.addEventListener('broglow:model-ready',() => {modelFailed = false;$('#lamp-fallback').hidden = true;renderModel();});
  window.addEventListener('broglow:model-error',() => {modelFailed = true;$('#lamp-fallback').hidden = false;renderModel();});
  const install = 'brew install broglow/tap/broglow\nbroglow install';
  function renderCopy() {
    if (copyState === 'done') {$('#copy-status').textContent = text('copy.done','Copied ✓');$('#copy-install').textContent = text('copy.done','Copied ✓');}
    if (copyState === 'manual') $('#copy-status').textContent = text('copy.manual','Press ⌘C or Ctrl+C.');
    $('#install-fallback')?.setAttribute('aria-label',text('copy.fallbackLabel','Installation commands to copy'));
  }
  $('#copy-install').addEventListener('click',async () => {
    try {await navigator.clipboard.writeText(install);copyState = 'done';}
    catch {
      copyState = 'manual';
      let fallback = $('#install-fallback');
      if (!fallback) {fallback = document.createElement('textarea');fallback.id = 'install-fallback';fallback.readOnly = true;$('.install-box').append(fallback);}
      fallback.value = install;fallback.focus();fallback.select();
    }
    renderCopy();
  });
  window.addEventListener('broglow:language',() => {
    renderHero();
    renderPhoto();
    renderModel();
    renderCopy();
    if (toast && !$('#toast').hidden) $('#toast').textContent = text(...toast);
  });
  renderHero();
  renderPhoto();
  renderModel();
})();
