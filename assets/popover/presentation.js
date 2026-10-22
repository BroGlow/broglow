(function (root, factory) {
  const presentation = factory();
  if (typeof module === 'object' && module.exports) module.exports = presentation;
  else root.BroGlowPresentation = presentation;
})(typeof window === 'undefined' ? globalThis : window, function () {
  'use strict';
  const escape = value => String(value).replace(/[&<>"']/g, character => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[character]);

  function tokens(n) {
    if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
    if (n >= 1e3) return Math.round(n / 1e3) + 'k';
    return String(n);
  }

  function agentMark(agent, showName) {
    const a11y = showName ? ' aria-hidden="true"' : ` role="img" aria-label="${escape(agent.label)}"`;
    return agent.icon
      ? `<svg class="agicon" viewBox="0 0 16 16"${a11y}><path d="${escape(agent.icon)}"/></svg>`
      : `<span class="badge"${a11y}>${escape(agent.badge || '')}</span>`;
  }

  function menuBarUsage(usage) {
    const agents = (usage?.agents || []).filter(agent => Number.isFinite(agent.limits?.[0]?.usedPercent));
    if (agents.length) return {
      html: agents.map(agent => `<span class="usage-agent">${agentMark(agent, false)} ${Math.round(agent.limits[0].usedPercent)}%</span>`).join('<span aria-hidden="true"> · </span>'),
      label: agents.map(agent => `${agent.label} ${Math.round(agent.limits[0].usedPercent)}%`).join(' · '),
    };
    const total = Number.isFinite(usage?.todayTokens) && usage.todayTokens > 0 ? `${tokens(usage.todayTokens)} tok` : '';
    return { html: total, label: total };
  }

  return { tokens, agentMark, menuBarUsage };
});
