const retry = new URL(import.meta.url).search;
if (!customElements.get("model-viewer")) {
  await import("./model-viewer-module-4.3.1.min.js" + retry);
}
if (!customElements.get("effect-composer")) {
  await import("./model-viewer-effects-1.5.0.min.js" + retry);
}
