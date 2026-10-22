(function (root, factory) {
  'use strict';
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.BroGlowPopoverDocument = factory();
})(typeof globalThis === 'object' ? globalThis : this, function () {
  'use strict';

  // Both hosts assemble this shell; the renderer only speaks __setState / window.ipc.
  function render({ template, styles, script, presentation, hostScript = '', platform = 'mac', icons } = {}) {
    for (const [name, asset] of Object.entries({ template, styles, script, presentation })) {
      if (typeof asset !== 'string' || !asset.trim()) throw new Error(`Missing popover ${name}`);
    }
    if (!['mac', 'win'].includes(platform)) throw new Error('Invalid popover platform');
    if (typeof hostScript !== 'string') throw new Error('Invalid popover host script');
    const slots = {
      __POPOVER_STYLE__: styles,
      __POPOVER_PRESENTATION__: presentation,
      __POPOVER_HOST__: hostScript ? `<script>${hostScript}</script>` : '',
      __POPOVER_SCRIPT__: script,
    };
    for (const slot of Object.keys(slots)) {
      if (template.split(slot).length !== 2) throw new Error(`Expected one popover slot: ${slot}`);
    }
    const replacements = { ...slots, __PLATFORM__: platform };
    for (const color of ['red', 'yellow', 'green', 'none']) {
      const icon = icons?.[color];
      if (typeof icon !== 'string' || !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(icon)) {
        throw new Error(`Missing or invalid popover ${color} icon`);
      }
      replacements[`__ICON_${color.toUpperCase()}__`] = icon;
    }
    let html = template;
    for (const [token, value] of Object.entries(replacements)) {
      html = html.replaceAll(token, () => value);
    }
    const unresolved = html.match(/__[A-Z][A-Z0-9_]*__/);
    if (unresolved) throw new Error(`Unresolved popover token: ${unresolved[0]}`);
    return html;
  }

  return { render };
});
