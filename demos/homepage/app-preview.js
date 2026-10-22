(() => {
  'use strict';
  const frame = document.querySelector('#app-preview');
  if (!frame) return;
  const signals = ['green', 'yellow', 'red'];
  let signal = signals.includes(document.body.dataset.signal) ? document.body.dataset.signal : 'green';
  let ready = false;
  let panelOpen = true;
  const token = crypto.randomUUID();
  const parentOrigin = window.location.origin;
  const emit = (name, detail) => window.dispatchEvent(new CustomEvent(name, { detail }));
  const push = () => {
    if (ready) frame.contentWindow.postMessage({ type: 'broglow:app-state', token, signal }, '*');
  };
  frame.setAttribute('sandbox', 'allow-scripts');
  window.addEventListener('broglow:app-state', event => {
    if (!signals.includes(event.detail?.signal)) return;
    signal = event.detail.signal;
    push();
  });
  window.addEventListener('broglow:app-visibility', event => {
    if (typeof event.detail?.open !== 'boolean') return;
    panelOpen = event.detail.open;
    if (ready) frame.contentWindow.postMessage({ type: 'broglow:app-visibility', token, open: panelOpen }, '*');
  });
  window.addEventListener('message', event => {
    if (event.source !== frame.contentWindow || event.origin !== 'null' || event.data?.token !== token) return;
    const message = event.data;
    if (message.type === 'broglow:app-ready') {
      ready = true;
      frame.dataset.ready = 'true';
      frame.contentWindow.postMessage({ type: 'broglow:app-visibility', token, open: panelOpen }, '*');
      push();
    } else if (message.type === 'broglow:app-resize' && Number.isFinite(message.height)) {
      const height = Math.max(100, Math.min(720, message.height));
      frame.style.height = `${height}px`;
      frame.parentElement.style.setProperty('--app-height', `${height}px`);
      emit(message.type, { width: 338, height });
    } else if (message.type === 'broglow:open-session' && ['codex', 'claude', 'opencode'].includes(message.session)) {
      emit(message.type, { session: message.session });
    } else if (message.type === 'broglow:app-action' && typeof message.action === 'string') {
      emit(message.type, { action: message.action });
    } else if (message.type === 'broglow:app-aggregate' && [...signals, 'none'].includes(message.signal)) {
      emit(message.type, { signal: message.signal });
    } else if (message.type === 'broglow:app-usage') {
      const usage = message.usage;
      if (usage === null || (Number.isFinite(usage?.todayTokens) && usage.todayTokens >= 0
        && Array.isArray(usage.agents) && usage.agents.length <= 4
        && usage.agents.every(agent => agent && typeof agent.label === 'string' && agent.label.length <= 100
          && typeof agent.badge === 'string' && agent.badge.length <= 10
          && (!agent.icon || typeof agent.icon === 'string' && agent.icon.length <= 300)
          && Array.isArray(agent.limits) && agent.limits.length <= 4 && agent.limits.every(limit => limit && Number.isFinite(limit.usedPercent)
            && limit.usedPercent >= 0 && limit.usedPercent <= 100)))) {
        emit(message.type, { usage });
      }
    } else if (message.type === 'broglow:app-activate') {
      emit(message.type);
    } else if (message.type === 'broglow:app-error') {
      frame.dataset.error = 'true';
      emit(message.type, { message: 'The BroGlow app preview could not render.' });
    }
  });

  // Run the shipped popover inside an isolated browser frame; replace only its native host.
  function previewHost(channelToken, hostOrigin, initialSignal) {
    const send = (type, detail = {}) => window.parent.postMessage({ type, token: channelToken, ...detail }, hostOrigin);
    for (const type of ['pointerdown', 'focusin']) {
      document.addEventListener(type, () => send('broglow:app-activate'), true);
    }
    let currentSignal = initialSignal;
    let closedAt = null;
    const agents = {
      codex: { label: 'Codex', badge: 'cx', icon: 'M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5z' },
      claude: { label: 'Claude', badge: 'cc', icon: 'M8 1.5v13M1.5 8h13M3.4 3.4l9.2 9.2M12.6 3.4l-9.2 9.2' },
      opencode: { label: 'opencode', badge: 'oc', icon: 'M5.5 2.5H3v11h2.5M10.5 2.5H13v11h-2.5' },
    };
    const model = {
      aggregate: currentSignal,
      sessions: [],
      settings: { showAgentNames: false, showDashboardButton: true, groupByProject: false,
        yellowMode: 'any_inactive', popoverResetEnabled: false, popoverResetMinutes: 5,
        autoAddForgottenUsb: true, usageEnabled: false, showUsageInMenuBar: true },
      lamp: { present: true, enabled: true, transport: 'usb', state: 'connected',
        detail: 'Simulated USB lamp. No hardware is connected by this website.', brightness_percent: 100 },
    };
    function render(resetNavigation = false) {
      const active = model.sessions.some(session => session.status === 'active');
      const inactive = model.sessions.some(session => session.status === 'inactive');
      model.aggregate = model.sessions.some(session => session.status === 'needs_help') ? 'red'
        : active && model.settings.yellowMode === 'active_wins' ? 'green'
        : inactive ? 'yellow' : active ? 'green' : 'none';
      model.usage = model.settings.usageEnabled ? {
        todayTokens: 1312400,
        agents: [
          { harness: 'claude', ...agents.claude, todayTokens: 1240000, limits: [
            { label: '5h block', usedPercent: 62, reset: '4:00 PM' },
            { label: 'Week', usedPercent: 31, reset: 'Fri' },
          ] },
          { harness: 'codex', ...agents.codex, todayTokens: 60000, limits: [
            { label: '5h block', usedPercent: 12, reset: '5:00 PM' },
            { label: 'Week', usedPercent: 44, reset: 'Sun' },
          ] },
          { harness: 'opencode', ...agents.opencode, todayTokens: 12400, limits: [] },
        ],
      } : null;
      window.__setState(model, resetNavigation);
      send('broglow:app-aggregate', { signal: model.aggregate });
      send('broglow:app-usage', { usage: model.settings.showUsageInMenuBar ? model.usage : null });
    }
    function setSignal(next) {
      currentSignal = next;
      model.sessions = [
        { id: 'codex', name: 'Test the checkout flow', project: 'shopfront', projectPath: '/demo/shopfront',
          status: next === 'red' ? 'needs_help' : next === 'yellow' ? 'inactive' : 'active' },
        { id: 'claude', name: 'Build the new homepage', project: 'good-things', projectPath: '/demo/good-things', status: 'active' },
        { id: 'opencode', name: 'Polish the little details', project: 'broglow', projectPath: '/demo/broglow', status: 'active' },
      ].map((session, index) => ({ ...session, agent: agents[session.id], host: { kind: 'terminal', name: 'Terminal' },
        lastUpdated: new Date(Date.now() - index * 60000).toISOString() }));
      render();
    }
    const toggles = { set_dashboard_button: 'showDashboardButton', set_agent_names: 'showAgentNames',
      set_group_by_project: 'groupByProject', set_auto_add_forgotten_usb: 'autoAddForgottenUsb',
      set_popover_reset: 'popoverResetEnabled', set_usage: 'usageEnabled', set_usage_in_menu_bar: 'showUsageInMenuBar' };
    window.ipc = { postMessage(raw) {
      const message = JSON.parse(raw);
      if (message.cmd === 'resize') { send('broglow:app-resize', { height: message.height }); return; }
      if (message.cmd === 'ready') {
        setSignal(currentSignal);
        send('broglow:app-ready');
        return;
      }
      if (message.cmd === 'focus') {
        if (model.sessions.some(session => session.id === message.id)) send('broglow:open-session', { session: message.id });
        return;
      }
      if (message.cmd === 'clear_session') model.sessions = model.sessions.filter(session => session.id !== message.id);
      else if (message.cmd === 'clear_paused') model.sessions = model.sessions.filter(session => session.status !== 'inactive');
      else if (message.cmd in toggles) model.settings[toggles[message.cmd]] = message.enabled === true;
      else if (message.cmd === 'set_yellow_mode') model.settings.yellowMode = message.mode;
      else if (message.cmd === 'set_popover_reset_minutes') model.settings.popoverResetMinutes = message.minutes;
      else if (message.cmd === 'set_led') {
        model.lamp.enabled = message.enabled;
        model.lamp.state = message.enabled ? 'connected' : 'disabled';
        model.lamp.detail = 'Simulated USB lamp. No hardware is connected by this website.';
      } else {
        if (['dashboard', 'quit', 'hide'].includes(message.cmd)) send('broglow:app-action', { action: message.cmd });
        return;
      }
      render();
      window.__configActionResult('');
    } };
    window.addEventListener('message', event => {
      if (event.source !== window.parent || event.origin !== hostOrigin || event.data?.token !== channelToken) return;
      if (event.data.type === 'broglow:app-visibility' && typeof event.data.open === 'boolean') {
        if (!event.data.open) {
          if (closedAt === null) closedAt = Date.now();
        } else {
          const reset = closedAt !== null && model.settings.popoverResetEnabled
            && Date.now() - closedAt >= model.settings.popoverResetMinutes * 60000;
          closedAt = null;
          if (reset) render(true);
        }
        return;
      }
      if (event.data.type === 'broglow:app-state' && ['green', 'yellow', 'red'].includes(event.data.signal)) {
        setSignal(event.data.signal);
      }
    });
    window.addEventListener('error', () => send('broglow:app-error'));
  }

  async function load() {
    try {
      const sourceUrl = new URL('../../assets/popover.html', document.baseURI);
      const response = await fetch(sourceUrl);
      if (!response.ok) throw new Error('Popover source unavailable');
      const template = await response.text();
      const [styles, script, presentation] = await Promise.all(['popover.css', 'popover.js', 'presentation.js'].map(async name => {
        const asset = await fetch(new URL(`popover/${name}`, sourceUrl));
        if (!asset.ok) throw new Error('Shared UI source unavailable');
        return asset.text();
      }));
      const icons = {};
      await Promise.all(['red', 'yellow', 'green', 'none'].map(async color => {
        const icon = await fetch(new URL(`icons/broglow-${color}.png`, sourceUrl));
        if (!icon.ok) throw new Error('Popover icon unavailable');
        const bytes = new Uint8Array(await icon.arrayBuffer());
        icons[color] = `data:image/png;base64,${btoa(String.fromCharCode(...bytes))}`;
      }));
      const shim = `(${previewHost.toString()})(${JSON.stringify(token)},${JSON.stringify(parentOrigin)},${JSON.stringify(signal)});`;
      frame.srcdoc = window.BroGlowPopoverDocument.render({ template, styles, script, presentation,
        hostScript: shim, platform: 'mac', icons });
    } catch {
      frame.dataset.error = 'true';
      emit('broglow:app-error', { message: 'The BroGlow app preview could not load its shared UI assets. Rebuild the static demo or serve it from the project root.' });
    }
  }
  load();
})();
