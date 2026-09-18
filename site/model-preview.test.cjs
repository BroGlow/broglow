const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');

function page(defined = false) {
  function element() {
    return {
      hidden: true, dataset: {}, events: {},
      addEventListener(name, callback) { this.events[name] = callback; },
      emit(name) { this.events[name](); },
      removeAttribute(name) { delete this[name]; },
      getAttribute() { return '25deg 75deg 110%'; },
      focus() { this.focused = true; },
      jumpCameraToGoal() { this.reset = true; },
    };
  }
  const ids = ['lamp-model', 'model-poster', 'model-load', 'model-reset', 'model-loading', 'model-error', 'model-help'];
  const nodes = Object.fromEntries(ids.map(id => [id, element()]));
  const scripts = [];
  const camera = element();
  nodes['lamp-model'].shadowRoot = { querySelector(selector) {
    assert.equal(selector, '[role="img"][tabindex="0"]');
    return camera;
  } };
  nodes['model-poster'].hidden = false;
  nodes['lamp-model'].dataset.modelSrc = 'assets/clawlight.glb';
  nodes['model-load'].dataset.viewerSrc = 'vendor/model-viewer-4.3.1.min.js';
  runInNewContext(readFileSync(`${__dirname}/model-preview.js`, 'utf8'), {
    customElements: { get: () => defined },
    document: {
      querySelector: selector => nodes[selector.slice(1)],
      createElement: element,
      head: { append: script => scripts.push(script) },
    },
  });
  return { nodes, scripts, camera, define() { defined = true; } };
}

const { nodes: n, scripts, camera, define } = page();
assert.equal(scripts.length, 0, 'No viewer module before explicit activation');
assert.equal(n['lamp-model'].src, undefined, 'No model download before activation');
assert.equal(n['model-poster'].hidden, false);
assert.equal(n['model-load'].hidden, false);
n['model-load'].emit('click');
assert.equal(scripts[0].type, 'module');
assert.equal(scripts[0].src, 'vendor/model-viewer-4.3.1.min.js');
assert.equal(n['model-loading'].hidden, false);
scripts[0].emit('error');
assert.equal(n['model-error'].hidden, false);
assert.equal(n['model-poster'].hidden, false);
assert.equal(n['model-load'].disabled, false);
n['model-load'].emit('click');
assert.equal(scripts[1].src, 'vendor/model-viewer-4.3.1.min.js?retry=1');
define();
scripts[1].emit('load');
assert.equal(n['lamp-model'].src, 'assets/clawlight.glb?retry=1');
n['lamp-model'].emit('load');
assert.equal(n['model-poster'].hidden, true);
assert.equal(n['model-reset'].hidden, false);
assert.equal(n['model-help'].hidden, false);
assert.equal(camera.focused, undefined, 'Wait until the component poster is dismissed');
n['lamp-model'].emit('poster-dismissed');
assert.equal(camera.focused, true, 'Activation focuses the keyboard camera target');
camera.focused = false;
n['model-reset'].emit('click');
assert.equal(n['lamp-model'].cameraOrbit, '25deg 75deg 110%');
assert.equal(n['lamp-model'].reset, true);
assert.equal(camera.focused, true, 'Reset returns keyboard control to the model');
n['lamp-model'].emit('error');
assert.equal(n['model-poster'].hidden, false, 'Model/WebGL failure restores the image');
assert.equal(n['lamp-model'].hidden, true);
assert.equal(n['model-load'].disabled, false);
assert.equal(n['model-help'].hidden, true);
n['model-load'].emit('click');
assert.equal(scripts.length, 2, 'A model retry reuses the loaded module');
assert.equal(n['lamp-model'].src, 'assets/clawlight.glb?retry=2');
const cached = page(true);
cached.nodes['model-load'].emit('click');
assert.equal(cached.scripts.length, 0, 'Reuse an already loaded component on retry');
assert.equal(cached.nodes['lamp-model'].src, 'assets/clawlight.glb');
console.log('PASS: 3D opt-in, module/model failure fallback, retry and camera reset');
