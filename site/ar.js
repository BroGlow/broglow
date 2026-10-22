(() => {
  const viewer = document.querySelector('#lamp-model');
  const button = document.querySelector('#model-space');
  const dialog = document.querySelector('#ar-dialog');
  if (!viewer || !button || !dialog) return;

  const nativeButton = document.querySelector('#model-ar');
  const close = document.querySelector('#ar-close');
  const qr = document.querySelector('#ar-qr');
  const link = document.querySelector('#ar-link');
  const publicLink = document.querySelector('#ar-public-link');
  const local = document.querySelector('#ar-local');
  const qrError = document.querySelector('#ar-qr-error');
  const reason = document.querySelector('#ar-reason');
  const scan = document.querySelector('#ar-scan');
  const mobile = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent)
    || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
  let reasonKey = '';
  let returnFocus;
  let previousOverflow;
  let active = false;
  let presenting = false;
  let showModelOnClose = false;

  function setActive(value) {
    if (active === value) return;
    active = value;
    window.dispatchEvent(new CustomEvent('broglow:ar-active', { detail: { active } }));
  }

  function phoneUrl(href) {
    const url = new URL(href);
    url.search = '';
    const language = window.BroGlowI18n?.language;
    if (language === 'en' || language === 'ru') url.searchParams.set('lang', language);
    url.hash = 'view-in-space';
    return url;
  }

  function render() {
    reason.hidden = !reasonKey;
    reason.textContent = reasonKey ? window.BroGlowI18n.t(reasonKey) : '';
    const url = phoneUrl(location.href);
    const reachable = /^https?:$/.test(url.protocol)
      && !/^(localhost|.*\.localhost|127\..*|\[::1\]|0\.0\.0\.0)$/.test(url.hostname);
    link.href = url.href;
    local.hidden = reachable;
    publicLink.hidden = reachable;
    publicLink.href = phoneUrl('https://broglow.github.io/broglow/').href;
    qr.hidden = scan.hidden = qrError.hidden = true;
    if (reachable) {
      try {
        const code = qrcode(0, 'M');
        code.addData(url.href);
        code.make();
        qr.src = code.createDataURL(6, 24);
        qr.hidden = scan.hidden = false;
      } catch {
        qrError.hidden = false;
      }
    }
  }

  function showDialog(key = '') {
    reasonKey = key;
    render();
    if (dialog.open) return;
    returnFocus = document.activeElement;
    previousOverflow = document.documentElement.style.overflow;
    dialog.showModal();
    document.documentElement.style.overflow = 'hidden';
    close.focus();
  }

  async function activate(event) {
    // Prevent model-viewer's slotted-button listener from launching a second time.
    event?.stopPropagation();
    if ((!mobile && event?.currentTarget !== nativeButton) || !viewer.canActivateAR) {
      showDialog(mobile ? 'ar.unavailable' : '');
      return;
    }
    setActive(true);
    try {
      await viewer.activateAR();
    } catch {
      presenting = false;
      showDialog('ar.failed');
    } finally {
      if (!presenting) setActive(false);
    }
  }
  button.addEventListener('click', activate);
  nativeButton?.addEventListener('click', activate);
  viewer.addEventListener('ar-status', event => {
    const status = event.detail?.status;
    presenting = status === 'session-started' || status === 'object-placed';
    setActive(presenting);
    if (status === 'failed') showDialog('ar.failed');
  });
  // Native viewers leave the page; returning must resume the scroll animation.
  window.addEventListener('focus', () => { if (!presenting) setActive(false); });
  window.addEventListener('pageshow', () => { if (!presenting) setActive(false); });

  close.addEventListener('click', () => dialog.close());
  dialog.addEventListener('close', () => {
    document.documentElement.style.overflow = previousOverflow;
    const camera = showModelOnClose && !viewer.inert
      ? viewer.shadowRoot?.querySelector('[role="img"][tabindex="0"]') : null;
    if (showModelOnClose) viewer.scrollIntoView({ block: 'center' });
    showModelOnClose = false;
    if (camera) camera.focus();
    else if (returnFocus?.isConnected) returnFocus.focus();
  });
  dialog.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right
      || event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
  });
  link.addEventListener('click', event => {
    event.preventDefault();
    showModelOnClose = true;
    dialog.close();
  });
  window.addEventListener('broglow:language', () => { if (dialog.open) render(); });
})();
