(() => {
  const root = document.documentElement;
  const params = new URLSearchParams(window.location.search);
  const themes = ['auto', 'light', 'dark'];
  const system = window.matchMedia('(prefers-color-scheme: dark)');
  const read = key => { try { return localStorage.getItem(key); } catch { return null; } };
  const valid = (value, options, fallback) => options.includes(value) ? value : fallback;
  let preference = valid(params.get('theme'), themes, valid(read('broglow-theme'), themes, 'auto'));

  function apply() {
    const theme = preference === 'auto' ? (system.matches ? 'dark' : 'light') : preference;
    root.dataset.theme = theme;
    root.dataset.themePreference = preference;
    root.dataset.palette = 'ember';
    root.style.colorScheme = theme;
    const select = document.querySelector('#theme');
    if (select) select.value = preference;
    window.dispatchEvent(new CustomEvent('broglow:appearance', { detail: { theme, palette: 'ember' } }));
  }

  function remember() {
    try { localStorage.setItem('broglow-theme', preference); } catch { /* Private browsing can disable storage. */ }
    const url = new URL(window.location.href);
    url.searchParams.set('theme', preference);
    url.searchParams.delete('palette');
    window.history.replaceState(null, '', url);
    apply();
  }

  function bind() {
    document.querySelector('#theme')?.addEventListener('change', event => {
      preference = valid(event.target.value, themes, preference);
      remember();
    });
    apply();
  }

  apply();
  system.addEventListener('change', () => { if (preference === 'auto') apply(); });
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', bind, { once: true });
  else bind();
})();
