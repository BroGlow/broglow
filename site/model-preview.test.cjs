const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');

async function run() {
const { PointLight, Mesh, BufferGeometry, Float32BufferAttribute } =
  await import('./vendor/three-0.183.0/three.module.min.js');

const glb = readFileSync(`${__dirname}/assets/broglow.glb`);
const gltf = JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)));
const html = readFileSync(`${__dirname}/index.html`, 'utf8');
const initialOrbit = html.match(/camera-orbit="([^"]+)"/)[1];
const lensIndices = { working: 4, idle: 5, 'needs-you': 6 };
for (const [number, index] of [4, 5, 6].entries()) {
  const node = gltf.nodes.find(node => node.name === `REFERENCE_ONLY_LED_${number}`);
  assert.equal(gltf.meshes[node.mesh].primitives[0].material, index, 'Keep preview material indices aligned with the exported lenses');
}

function page(defined = false, href = 'https://example.com/broglow/', qrFailure = false, lateEffects = false) {
  let effectsDefined = defined;
  const document = { activeElement: null };
  function element() {
    return {
      hidden: true, dataset: {}, events: {},
      addEventListener(name, callback) { this.events[name] = callback; },
      emit(name, event = { target: this }) {
        if (name === 'click') document.activeElement = event.target;
        this.events[name]?.(event);
      },
      removeAttribute(name) { delete this[name]; },
      getAttribute() { return initialOrbit; },
      focus() { this.focused = true; document.activeElement = this; },
      jumpCameraToGoal() { this.reset = true; },
      showModal() { this.open = true; },
      close() { this.open = false; },
      getBoundingClientRect() { return { left: 100, right: 500, top: 100, bottom: 600 }; },
      activateAR() { assert.fail('Opening the phone link must not launch AR without a tap'); },
    };
  }
  const ids = ['lamp-model', 'model-poster', 'model-load', 'model-reset', 'model-loading', 'model-error', 'model-help',
    'model-space', 'ar-dialog', 'ar-close', 'ar-qr', 'ar-link', 'ar-local', 'ar-qr-error',
    'assembly-model', 'assembly-loading', 'assembly-error', 'assembly-retry', 'assembly-reset'];
  const nodes = Object.fromEntries(ids.map(id => [id, element()]));
  const assemblyTag = html.match(/<model-viewer id="assembly-model"[^>]+>/)[0];
  nodes['assembly-model'].getAttribute = name => assemblyTag.match(new RegExp(`${name}="([^"]+)"`))?.[1];
  const signals = element();
  const radios = [...Object.keys(lensIndices), 'off'].map((value, index) => ({
    ...element(), value, name: 'signal', checked: index === 0,
    matches: selector => selector === 'input[name="signal"]',
  }));
  signals.querySelector = selector => {
    assert.equal(selector, 'input:checked');
    return radios.find(radio => radio.checked);
  };
  signals.contains = target => target === signals || radios.includes(target);
  const viewer = nodes['lamp-model'];
  const queueRender = function() { this.renders++; };
  const composer = { ...element(), selection: lateEffects ? undefined : new Set(), renders: 0,
    queueRender: lateEffects ? undefined : queueRender,
  };
  viewer.composer = composer;
  viewer.querySelector = selector => {
    assert.equal(selector, 'effect-composer');
    return composer;
  };
  function populateSelection() {
    composer.selection ??= new Set();
    composer.queueRender ??= queueRender;
    if (composer.selection.size) return;
    for (const node of gltf.nodes.filter(node => node.mesh !== undefined)) {
      const primitive = gltf.meshes[node.mesh].primitives[0];
      const bounds = gltf.accessors[primitive.attributes.POSITION];
      const geometry = new BufferGeometry();
      geometry.setAttribute('position', new Float32BufferAttribute([...bounds.min, ...bounds.max], 3));
      const mesh = new Mesh(geometry);
      mesh.name = node.name;
      composer.selection.add(mesh);
    }
    composer.emit('updated-selection');
  }
  const emitViewer = viewer.emit;
  viewer.emit = function(name, event) {
    if (name === 'before-render' || name === 'load') {
      this.loaded = name === 'load';
      this.model ??= { materials: gltf.materials.map(source => {
        const material = {
          writes: 0, emissiveFactor: [0, 0, 0], emissiveStrength: 1,
          setEmissiveFactor(value) { this.emissiveFactor = Array.from(value); this.writes++; },
          setEmissiveStrength(value) { this.emissiveStrength = value; this.writes++; },
          pbrMetallicRoughness: {
            ...source.pbrMetallicRoughness,
            setBaseColorFactor(value) { this.baseColorFactor = Array.from(value); material.writes++; },
          },
        };
        return material;
      }) };
    } else if (name === 'error') {
      this.loaded = false;
      this.model = undefined;
      composer.selection?.clear();
    }
    if (name === 'load' && !composer.selection?.size) {
      emitViewer.call(this, 'before-render');
      populateSelection();
    }
    emitViewer.call(this, name, event);
    // model-preview's listener is registered before the effects component's.
    if (name === 'before-render') populateSelection();
  };
  const scripts = [];
  const qrValues = [];
  const camera = element();
  nodes['lamp-model'].shadowRoot = { querySelector(selector) {
    assert.equal(selector, '[role="img"][tabindex="0"]');
    return camera;
  } };
  nodes['assembly-model'].shadowRoot = { querySelector: () => nodes['assembly-model'] };
  nodes['model-poster'].hidden = false;
  nodes['lamp-model'].dataset.modelSrc = 'assets/broglow.glb';
  nodes['model-load'].dataset.viewerSrc = html.match(/data-viewer-src="([^"]+)"/)[1];
  Object.assign(document, {
    querySelector: selector => selector === '.signal-demo' ? signals :
      ['effect-composer', '#lamp-model effect-composer'].includes(selector) ? composer : nodes[selector.slice(1)],
    createElement: element,
    head: { append: script => scripts.push(script) },
  });
  const source = readFileSync(`${__dirname}/model-preview.js`, 'utf8')
    .replace('import { PointLight } from "three";', '');
  runInNewContext(source, {
    URL, PointLight,
    location: new URL(href),
    qrcode: () => ({
      addData(value) { qrValues.push(value); },
      make() { if (qrFailure) throw new Error('QR generation failed'); },
      createDataURL: () => 'data:image/gif;base64,QR',
    }),
    customElements: { get: name => name === 'model-viewer' ? defined : name === 'effect-composer' && effectsDefined },
    document,
  });
  return { nodes, scripts, camera, qrValues, document,
    define() { defined = effectsDefined = true; },
    defineViewerOnly() { defined = true; effectsDefined = false; },
    select(value, event = 'change') {
      for (const radio of radios) radio.checked = radio.value === value;
      const target = radios.find(radio => radio.checked);
      document.activeElement = target;
      signals.emit(event, { target });
      return target;
    },
  };
}

function assertSignal(viewer, mode) {
  for (const [index, material] of viewer.model.materials.entries()) {
    if (![4, 5, 6].includes(index)) {
      if (![0, 1].includes(index)) assert.equal(material.writes, 0, 'Electronics materials stay untouched');
      continue;
    }
    assert.equal(material.emissiveFactor.some(value => value > 0), index === lensIndices[mode],
      `${mode}: only the selected lens emits light`);
    if (index === lensIndices[mode]) assert.ok(material.emissiveStrength > 0 && material.emissiveStrength <= 1.5,
      'Active lens glows without the previous eye-searing emission');
    else {
      assert.equal(material.emissiveStrength, 0, 'Inactive lenses emit no glow');
      assert.ok(Math.max(...material.pbrMetallicRoughness.baseColorFactor.slice(0, 3)) < 0.1,
        'Inactive lenses stay visibly dark');
    }
  }
  const lights = [];
  for (const mesh of viewer.composer.selection) mesh.traverse(object => {
    if (object.isPointLight) lights.push(object);
  });
  assert.ok(lights.length <= 1, 'Repeated choices and loads reuse one real light');
  if (mode === 'off') {
    assert.ok(lights.every(light => light.intensity === 0), 'Off extinguishes the source as well as the lenses');
    return;
  }
  assert.equal(lights.length, 1, 'The selected lens has a real source that illuminates nearby surfaces');
  const [light] = lights;
  assert.equal(light.parent.name, `REFERENCE_ONLY_LED_${lensIndices[mode] - 4}`);
  assert.deepEqual(light.color.toArray(), viewer.model.materials[lensIndices[mode]].emissiveFactor,
    'The nearby color cast matches the selected lens');
  assert.ok(light.intensity > 0 && light.intensity <= 0.1, 'Source adds a restrained color cast to nearby surfaces');
  assert.equal(light.distance, 0.035, 'The source only reaches nearby geometry');
  assert.equal(light.decay, 2, 'The source uses inverse-square attenuation');
  const bounds = light.parent.geometry.boundingBox;
  for (const axis of ['x', 'y']) assert.equal(light.position[axis], (bounds.min[axis] + bounds.max[axis]) / 2);
  assert.ok(light.position.z >= bounds.min.z && light.position.z <= bounds.max.z + 0.002,
    'Source sits in or just ahead of the emitting lens');
  assert.ok(viewer.composer.renders > 0, 'Source changes request a new frame');
  return light;
}

function assertEnclosure(viewer) {
  for (const index of [0, 1]) {
    const color = viewer.model.materials[index].pbrMetallicRoughness.baseColorFactor;
    const original = gltf.materials[index].pbrMetallicRoughness.baseColorFactor;
    assert.ok(color.slice(0, 3).reduce((sum, value) => sum + value, 0) <
      original.slice(0, 3).reduce((sum, value) => sum + value, 0), 'Enclosure matches the darker original preview');
  }
}

const deferredEffects = page(true, undefined, false, true);
const deferredViewer = deferredEffects.nodes['lamp-model'];
assert.equal(deferredViewer.composer.selection, undefined, 'Effects may not be upgraded at first model render');
assert.equal(deferredViewer.composer.queueRender, undefined);
assert.doesNotThrow(() => deferredViewer.emit('before-render'), 'Initial model render tolerates unupgraded effects');
assertSignal(deferredViewer, 'working');
deferredViewer.emit('load');
assertSignal(deferredViewer, 'working');

const startup = page(true);
const startupViewer = startup.nodes['lamp-model'];
assert.equal(startupViewer.src, 'assets/broglow.glb', 'Initial Working preview loads without a click');
startupViewer.emit('before-render');
const startupLight = assertSignal(startupViewer, 'working');
startupViewer.composer.emit('updated-selection');
assert.equal(assertSignal(startupViewer, 'working'), startupLight, 'Effects refresh reuses the existing source');
assertEnclosure(startupViewer);
const firstFrameMaterials = startupViewer.model.materials;
startupViewer.emit('load');
assert.equal(startupViewer.model.materials, firstFrameMaterials, 'Load preserves the pre-render model');
assertSignal(startupViewer, 'working');
startupViewer.emit('poster-dismissed');
assert.equal(startup.camera.focused, undefined, 'Automatic startup never takes keyboard focus');

const preview = page();
const previewViewer = preview.nodes['lamp-model'];
assert.equal(preview.scripts.length, 1, 'The viewer module starts loading on page load');
preview.select('working', 'click');
assert.equal(preview.scripts.length, 1, 'Clicking the already-selected Working mode reuses the startup load');
preview.select('idle');
preview.select('needs-you', 'click');
assert.equal(preview.scripts.length, 1, 'Rapid choices while loading reuse the pending viewer module');
preview.define();
preview.scripts[0].emit('load');
preview.select('off');
previewViewer.emit('before-render');
assertSignal(previewViewer, 'off');
previewViewer.emit('load');
assertSignal(previewViewer, 'off');
previewViewer.emit('poster-dismissed');
assert.equal(preview.camera.focused, undefined, 'Asynchronous loading keeps focus on the signal controls');
const enclosureWrites = previewViewer.model.materials.slice(0, 2).map(material => material.writes);
let previewLight;
for (const mode of ['working', 'idle', 'needs-you', 'off', 'working']) {
  preview.select(mode);
  const light = assertSignal(previewViewer, mode);
  if (light) {
    previewLight ??= light;
    assert.equal(light, previewLight, 'Changing modes repositions the same source');
  }
  assert.deepEqual(previewViewer.model.materials.slice(0, 2).map(material => material.writes), enclosureWrites,
    'Signal changes leave the initialized enclosure alone');
}
preview.nodes['model-reset'].emit('click');
assertSignal(previewViewer, 'working');
previewViewer.emit('error');
preview.select('needs-you', 'click');
assert.equal(previewViewer.src, 'assets/broglow.glb?retry=1', 'A signal choice retries a failed model');
previewViewer.emit('load');
assert.equal(assertSignal(previewViewer, 'needs-you'), previewLight, 'Retry reattaches the same source to the new model');

const moduleRetry = page();
moduleRetry.select('idle', 'click');
moduleRetry.defineViewerOnly();
moduleRetry.scripts[0].emit('error');
moduleRetry.select('needs-you', 'click');
assert.equal(moduleRetry.scripts.length, 2, 'Retry loads missing effects even when model-viewer already registered');
assert.equal(moduleRetry.nodes['lamp-model'].src, undefined, 'The model waits for effects before retrying');
assert.equal(moduleRetry.scripts[1].src, 'vendor/model-viewer-with-effects.js?retry=1');
moduleRetry.define();
moduleRetry.scripts[1].emit('load');
moduleRetry.nodes['lamp-model'].emit('load');
assertSignal(moduleRetry.nodes['lamp-model'], 'needs-you');

const { nodes: n, scripts, camera, define } = page();
assert.equal(scripts.length, 1, 'Startup loads the viewer without explicit activation');
assert.equal(n['lamp-model'].src, undefined, 'Model download waits for the viewer module');
assert.equal(n['model-poster'].hidden, false);
assert.equal(n['model-load'].hidden, true, 'Retry only appears after a loading failure');
n['model-load'].emit('click');
assert.equal(scripts[0].type, 'module');
assert.equal(scripts[0].src, 'vendor/model-viewer-with-effects.js');
assert.equal(n['model-loading'].hidden, false);
scripts[0].emit('error');
assert.equal(n['model-error'].hidden, false);
assert.equal(n['model-poster'].hidden, false);
assert.equal(n['model-load'].disabled, false);
n['model-load'].emit('click');
assert.equal(scripts[1].src, 'vendor/model-viewer-with-effects.js?retry=1');
define();
scripts[1].emit('load');
assert.equal(n['lamp-model'].src, 'assets/broglow.glb?retry=1');
n['lamp-model'].emit('load');
assertSignal(n['lamp-model'], 'working');
assert.equal(n['model-poster'].hidden, true);
assert.equal(n['model-reset'].hidden, false);
assert.equal(n['model-help'].hidden, false);
assert.equal(camera.focused, undefined, 'Wait until the component poster is dismissed');
n['lamp-model'].emit('poster-dismissed');
assert.equal(camera.focused, true, 'Activation focuses the keyboard camera target');
camera.focused = false;
n['model-reset'].emit('click');
assert.equal(n['lamp-model'].cameraOrbit, initialOrbit);
assert.equal(n['lamp-model'].reset, true);
assert.equal(camera.focused, true, 'Reset returns keyboard control to the model');
n['lamp-model'].emit('error');
assert.equal(n['model-poster'].hidden, false, 'Model/WebGL failure restores the image');
assert.equal(n['lamp-model'].hidden, true);
assert.equal(n['model-load'].disabled, false);
assert.equal(n['model-help'].hidden, true);
n['model-load'].emit('click');
assert.equal(scripts.length, 2, 'A model retry reuses the loaded module');
assert.equal(n['lamp-model'].src, 'assets/broglow.glb?retry=2');
const cached = page(true);
cached.nodes['model-load'].emit('click');
assert.equal(cached.scripts.length, 0, 'Reuse an already loaded component on retry');
assert.equal(cached.nodes['lamp-model'].src, 'assets/broglow.glb');

const space = page(false, 'https://example.com/project/index.html?token=private#signals');
assert.equal(space.nodes['model-space'].hidden, false, 'Phone handoff is available before loading 3D');
space.nodes['model-space'].emit('click');
assert.equal(space.nodes['ar-dialog'].open, true);
assert.equal(space.nodes['ar-link'].href, 'https://example.com/project/index.html#view-in-space');
assert.deepEqual(space.qrValues, [space.nodes['ar-link'].href], 'QR retains the deployment path and drops query parameters');
assert.equal(space.nodes['ar-qr'].src, 'data:image/gif;base64,QR');
assert.equal(space.nodes['ar-qr'].hidden, false);
assert.equal(space.nodes['ar-local'].hidden, true);
assert.equal(space.nodes['ar-qr-error'].hidden, true);
assert.equal(space.scripts.length, 1, 'Opening the QR modal does not duplicate the startup viewer load');
space.nodes['ar-dialog'].emit('click', { target: space.nodes['ar-qr'] });
assert.equal(space.nodes['ar-dialog'].open, true, 'Clicking modal content keeps it open');
space.nodes['ar-dialog'].emit('click', { target: space.nodes['ar-dialog'], clientX: 110, clientY: 110 });
assert.equal(space.nodes['ar-dialog'].open, true, 'Clicking dialog padding keeps it open');
space.nodes['ar-dialog'].emit('click', { target: space.nodes['ar-dialog'], clientX: 10, clientY: 10 });
assert.equal(space.nodes['ar-dialog'].open, false, 'Backdrop click closes the modal');
assert.match(html,
  /<form method="dialog"[^>]*>\s*<button id="ar-close"[^>]*type="submit"/,
  'Close button uses the native dialog form');
space.nodes['model-space'].emit('click');
let prevented = false;
space.nodes['ar-link'].emit('click', { preventDefault() { prevented = true; } });
assert.equal(prevented, true, 'Opening on this device preserves the current page');
assert.equal(space.nodes['ar-dialog'].open, false);
assert.equal(space.scripts.length, 1, 'Opening on this device reuses the startup model load');
space.nodes['model-load'].emit('click');
assert.equal(space.scripts.length, 1, 'Do not duplicate a pending model load');

for (const href of ['http://localhost:8080/site/', 'http://preview.localhost/site/',
  'http://127.0.0.1/site/', 'http://127.2.3.4/site/', 'http://[::1]/site/',
  'http://0.0.0.0/site/', 'file:///project/site/index.html']) {
  const local = page(false, href);
  local.nodes['model-space'].emit('click');
  assert.equal(local.nodes['ar-dialog'].open, true, href);
  assert.equal(local.nodes['ar-qr'].hidden, true, `${href} cannot be scanned from another device`);
  assert.equal(local.nodes['ar-local'].hidden, false, href);
  assert.equal(local.qrValues.length, 0, 'Do not generate an unusable QR');
}

const assemblyPage = page(true);
const assemblyNodes = assemblyPage.nodes;
const assembly = assemblyNodes['assembly-model'];
assert.match(html, /id="assembly-model"[^>]+camera-controls[^>]+touch-action="pan-y"/);
assert.doesNotMatch(html, /assembly\.png/, 'The exploded still image is removed');
assert.equal(assemblyNodes['assembly-loading'].hidden, false);
assembly.emit('load');
assert.equal(assemblyNodes['assembly-loading'].hidden, true);
assert.equal(assemblyNodes['assembly-reset'].hidden, false);
assemblyNodes['assembly-reset'].emit('click');
assert.equal(assembly.cameraOrbit, assembly.getAttribute('camera-orbit'));
assert.equal(assembly.cameraTarget, 'auto auto auto');
assert.equal(assembly.reset, true);
assert.equal(assembly.focused, true, 'Reset restores keyboard camera control');
assembly.emit('error');
assert.equal(assemblyNodes['assembly-error'].hidden, false);
assert.equal(assemblyNodes['assembly-retry'].hidden, false);
assert.equal(assemblyNodes['assembly-reset'].hidden, true);
assemblyNodes['assembly-retry'].emit('click');
assert.equal(assembly.src, 'assets/assembly.glb?retry=1');
assert.equal(assemblyPage.scripts.length, 0, 'Assembly retry reuses the shared component');
assembly.emit('load');
assert.equal(assemblyNodes['assembly-error'].hidden, true);
const missingComponent = page();
missingComponent.scripts[0].emit('error');
assert.equal(missingComponent.nodes['assembly-error'].hidden, false);
missingComponent.nodes['assembly-retry'].emit('click');
assert.equal(missingComponent.scripts.length, 2, 'Assembly retry reloads a failed shared component');
assert.equal(missingComponent.scripts[1].src, 'vendor/model-viewer-with-effects.js?retry=1');
const assemblyGlb = readFileSync(`${__dirname}/assets/assembly.glb`);
const assemblyGltf = JSON.parse(assemblyGlb.subarray(20, 20 + assemblyGlb.readUInt32LE(12)));
assert.deepEqual(assemblyGltf.nodes.filter(n => n.mesh !== undefined).map(n => n.name).sort(),
  gltf.nodes.filter(n => n.mesh !== undefined).map(n => n.name).sort(), 'Explorer preserves every assembly part');
console.log('PASS: assembly explorer inventory, reset, loading failures and shared-component retry');

const qrError = page(false, 'https://example.com/', true);
qrError.nodes['model-space'].emit('click');
assert.equal(qrError.nodes['ar-dialog'].open, true);
assert.equal(qrError.nodes['ar-qr'].hidden, true);
assert.equal(qrError.nodes['ar-qr-error'].hidden, false);
assert.equal(qrError.nodes['ar-link'].href, 'https://example.com/#view-in-space', 'QR failure keeps the usable link');

const phone = page(false, 'https://example.com/broglow/#view-in-space');
assert.equal(phone.scripts.length, 1, 'Scanning the phone link loads 3D immediately');
phone.define();
phone.scripts[0].emit('load');
assert.equal(phone.nodes['lamp-model'].src, 'assets/broglow.glb');
phone.nodes['lamp-model'].emit('load');
assert.equal(phone.nodes['ar-dialog'].open, undefined, 'The phone link shows the model, not another QR modal');
assert.equal(page(false, 'https://example.com/#signals').scripts.length, 1, 'All sections initialize the live model');

assert.ok(Number(html.match(/<bloom-effect[^>]*strength="([^"]+)"/)[1]) <= 0.3,
  'Bloom stays subtle rather than amplifying the whole lens');
assert.match(html, /<script[^>]*type="module"[^>]*src="model-preview.js"|<script[^>]*src="model-preview.js"[^>]*type="module"/,
  'The preview imports its light from the shared Three module');
console.log('PASS: automatic preview, subtle glow and real light source, load/retry/focus/reset, QR modal and phone deep link');
}

run().catch(error => { console.error(error); process.exitCode = 1; });
