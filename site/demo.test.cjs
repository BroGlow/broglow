const assert = require("node:assert/strict");
const { readFileSync, existsSync } = require("node:fs");
const { resolve } = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const source = readFileSync(resolve(__dirname, "lamp.js"), "utf8");
const modelFile = readFileSync(resolve(__dirname, "./assets/broglow.glb"));
const gltf = JSON.parse(modelFile.subarray(20, 20 + modelFile.readUInt32LE(12)).toString());
const lensIndices = [0, 1, 2].map(index => {
  const node = gltf.nodes.find(node => node.name === `REFERENCE_ONLY_LED_${index}`);
  return gltf.meshes[node.mesh].primitives[0].material;
});

class DetailEvent extends Event {
  constructor(type, detail) {
    super(type);
    this.detail = detail;
  }
}

function preview({ reducedMotion = false, importError = false, interactive = false, width = 1280 } = {}) {
  function element(dataset = {}) {
    const node = new EventTarget();
    node.dataset = dataset;
    node.attributes = {};
    node.classes = new Set();
    node.classList = { toggle(name, active) { active ? node.classes.add(name) : node.classes.delete(name); } };
    node.setAttribute = (name, value) => { node.attributes[name] = value; };
    return node;
  }
  let sectionTop = 500;
  const steps = ["green", "yellow", "red"].map((signal, index) => {
    const step = element({ modelStep: signal, orbit: [`-30deg 77deg 110%`, `0deg 74deg 100%`, `30deg 71deg 90%`][index] });
    step.getBoundingClientRect = () => ({ top: sectionTop + index * 160, height: 80 });
    return step;
  });
  const controls = ["green", "yellow", "red"].map(signal => element({ demoSignal: signal }));
  const demo = element();
  demo.getBoundingClientRect = () => ({ top: sectionTop, height: 700 });
  demo.querySelectorAll = selector => selector === "[data-model-step]" ? steps : controls;
  const brightness = element();
  brightness.value = "75";
  const output = element();
  const frames = new Map();
  let frameID = 0;
  let intersection;
  const viewer = new EventTarget();
  viewer.dataset = {};
  viewer.getBoundingClientRect = () => ({ top: sectionTop + 120, height: 320 });
  viewer.removed = new Set();
  viewer.removeAttribute = name => viewer.removed.add(name);
  const body = { dataset: {} };
  const media = new EventTarget();
  media.matches = reducedMotion;
  const browser = new EventTarget();
  browser.matchMedia = () => media;
  browser.innerHeight = 800;
  browser.innerWidth = width;
  const materials = gltf.materials.map(() => {
    const material = {};
    material.pbrMetallicRoughness = {
      setBaseColorFactor: value => { material.base = Array.from(value); },
    };
    material.setEmissiveFactor = value => { material.emissive = Array.from(value); };
    material.setEmissiveStrength = value => { material.strength = value; };
    return material;
  });

  // Only the browser's module loader is stubbed; the preview logic runs unchanged.
  assert.ok(source.includes('import("./vendor/model-viewer-module-4.3.1.min.js")'));
  vm.runInNewContext(source.replace("import(", "loadViewer("), {
    document: { body, querySelector: selector => selector === "#lamp-model" ? viewer
      : !interactive ? null : { "#model-demo": demo, "#lamp-brightness": brightness, "#brightness-value": output }[selector] },
    IntersectionObserver: class {
      constructor(callback) { intersection = callback; }
      observe(target) { assert.equal(target, demo); }
    },
    requestAnimationFrame: callback => { frames.set(++frameID, callback); return frameID; },
    cancelAnimationFrame: id => frames.delete(id),
    window: browser,
    CustomEvent: class extends DetailEvent {
      constructor(type, options) { super(type, options?.detail); }
    },
    loadViewer: path => {
      assert.ok(existsSync(resolve(__dirname, path)), "self-hosted model-viewer exists");
      return importError ? Promise.reject(new Error("module unavailable")) : Promise.resolve();
    },
  });

  return {
    viewer, body, media, materials, controls, demo, steps, brightness, output, browser,
    scroll(top) {
      sectionTop = top;
      if (!frames.size) intersection([{ isIntersecting: true }]);
      const current = [...frames.values()];
      frames.clear();
      current.forEach(callback => callback());
    },
    hide() { intersection([{ isIntersecting: false }]); },
    pendingFrames: () => frames.size,
    send: (type, detail) => browser.dispatchEvent(new DetailEvent(type, detail)),
    load() {
      viewer.model = { materials };
      viewer.dispatchEvent(new Event("load"));
    },
    strengths: () => lensIndices.map(index => materials[index].strength),
  };
}

test("signals illuminate exactly their physical GLB lens; off extinguishes all", () => {
  const demo = preview();
  assert.deepEqual(lensIndices, [4, 5, 6]);
  demo.load();
  assert.deepEqual(demo.strengths(), [1.2, 0, 0], "green is the default");
  for (const [signal, expected] of [
    ["yellow", [0, 1.2, 0]], ["red", [0, 0, 1.2]],
    ["off", [0, 0, 0]], ["green", [1.2, 0, 0]],
  ]) {
    demo.send("broglow:signal", { signal });
    assert.deepEqual(demo.strengths(), expected, signal);
    for (const index of lensIndices.filter(index => !demo.materials[index].strength)) {
      assert.deepEqual(demo.materials[index].emissive, [0, 0, 0], "inactive lenses do not emit");
    }
  }
  demo.send("broglow:signal", { signal: "unknown" });
  assert.equal(demo.viewer.dataset.signal, "green");
});

test("a signal received before the model loads survives initialization", () => {
  const demo = preview();
  demo.send("broglow:signal", { signal: "yellow" });
  demo.load();
  assert.deepEqual(demo.strengths(), [0, 1.2, 0]);
  assert.equal(demo.body.dataset.modelReady, "true");
  assert.equal(demo.viewer.dataset.modelReady, "true");
});

test("model errors expose fallback flags and clear stale ready flags", () => {
  const demo = preview();
  assert.equal(demo.viewer.inert, true, 'loading viewer cannot receive invisible focus');
  demo.load();
  assert.equal(demo.viewer.inert, false);
  demo.viewer.dispatchEvent(new Event("error"));
  assert.equal(demo.viewer.inert, true);
  for (const host of [demo.viewer, demo.body]) {
    assert.equal(host.dataset.modelError, "true");
    assert.equal(host.dataset.modelReady, undefined);
  }
  demo.load();
  assert.equal(demo.viewer.inert, false, 'a recovered viewer is interactive again');
});

test("a failed viewer module exposes fallback flags", async () => {
  const demo = preview({ importError: true });
  await Promise.resolve();
  assert.equal(demo.body.dataset.modelError, "true");
  assert.equal(demo.viewer.dataset.modelError, "true");
});

test("reduced motion suppresses camera updates and auto-rotation", () => {
  const demo = preview();
  demo.send("broglow:camera", { orbit: "12deg 78deg 0.3m" });
  assert.equal(demo.viewer.cameraOrbit, "12deg 78deg 0.3m");
  demo.media.matches = true;
  demo.media.dispatchEvent(new Event("change"));
  assert.ok(demo.viewer.removed.has("auto-rotate"));
  demo.send("broglow:camera", { orbit: "40deg 80deg 0.3m" });
  assert.equal(demo.viewer.cameraOrbit, "12deg 78deg 0.3m");

  const initialReduced = preview({ reducedMotion: true });
  initialReduced.send("broglow:camera", { orbit: "40deg 80deg 0.3m" });
  assert.equal(initialReduced.viewer.cameraOrbit, undefined);
  assert.ok(initialReduced.viewer.removed.has("auto-rotate"));
});


test("normal-flow scroll changes the physical lens and interpolates model angles", () => {
  const demo = preview({ interactive: true });
  demo.load();
  demo.scroll(400);
  assert.equal(demo.viewer.dataset.signal, "green");
  assert.equal(demo.viewer.cameraOrbit, "-30.00deg 77.00deg 110.00%");
  demo.scroll(200);
  assert.equal(demo.viewer.dataset.signal, "yellow");
  assert.deepEqual(demo.strengths(), [0, 1.2, 0]);
  assert.equal(demo.viewer.cameraOrbit, "0.00deg 74.00deg 100.00%");
  demo.scroll(120);
  assert.equal(demo.viewer.cameraOrbit, "15.00deg 72.50deg 95.00%");
  demo.scroll(40);
  assert.equal(demo.viewer.dataset.signal, "red");
  assert.deepEqual(demo.strengths(), [0, 0, 1.2]);
  demo.hide();
  assert.equal(demo.pendingFrames(), 0, "offscreen demos do not run animation frames");
});

test("scroll animation pauses during AR and resumes after returning to the page", () => {
  const demo = preview({ interactive: true });
  demo.load();
  demo.scroll(400);
  const orbit = demo.viewer.cameraOrbit;
  demo.send("broglow:ar-active", { active: true });
  demo.scroll(40);
  assert.equal(demo.viewer.dataset.signal, "green");
  assert.equal(demo.viewer.cameraOrbit, orbit);
  demo.send("broglow:ar-active", { active: false });
  demo.scroll(40);
  assert.equal(demo.viewer.dataset.signal, "red");
  assert.notEqual(demo.viewer.cameraOrbit, orbit);
});

test("manual model controls persist until scrolling resumes and brightness affects emission", () => {
  const demo = preview({ interactive: true });
  const notifications = [];
  demo.browser.addEventListener("broglow:model-signal", event => notifications.push(event.detail));
  demo.load();
  demo.scroll(400);
  demo.controls[2].dispatchEvent(new Event("click"));
  demo.scroll(400);
  assert.equal(demo.viewer.dataset.signal, "red", "a frame without scrolling preserves the user's choice");
  assert.equal(demo.controls[2].attributes["aria-pressed"], "true");
  assert.ok(demo.steps[2].classes.has("active"));
  demo.brightness.value = "0";
  demo.brightness.dispatchEvent(new Event("input"));
  assert.deepEqual(demo.strengths(), [0, 0, 0]);
  assert.equal(demo.output.textContent, "0%");
  demo.brightness.value = "100";
  demo.brightness.dispatchEvent(new Event("input"));
  assert.ok(Math.abs(demo.materials[6].strength - 1.6) < 0.00001);
  assert.equal(demo.output.textContent, "100%");
  assert.deepEqual(JSON.parse(JSON.stringify(notifications.at(-1))), { signal: "red", brightness: 1 });
  demo.scroll(390);
  assert.equal(demo.viewer.dataset.signal, "green", "native scrolling returns to the narrative");
  demo.send("broglow:signal", { signal: "red" });
  assert.equal(demo.viewer.dataset.signal, "green", "hero state does not override the dedicated model demo");
});

test("reduced-motion scroll demonstrates LED states without rotating the model", () => {
  const demo = preview({ interactive: true, reducedMotion: true });
  demo.load();
  demo.scroll(40);
  assert.equal(demo.viewer.dataset.signal, "red");
  assert.equal(demo.viewer.cameraOrbit, undefined);
  assert.equal(demo.viewer.interpolationDecay, 0);
  demo.controls[1].dispatchEvent(new Event("click"));
  assert.deepEqual(demo.strengths(), [0, 1.2, 0]);
  assert.equal(demo.viewer.cameraOrbit, undefined);
});


for (const width of [390, 723, 900]) test(`stacked ${width}px layouts demonstrate all states while the model remains visible`, () => {
  const demo = preview({ interactive: true, width });
  demo.load();
  for (const [top, signal, orbit] of [
    [320, "green", "-30.00deg 77.00deg 110.00%"],
    [120, "yellow", "0.00deg 74.00deg 100.00%"],
    [-80, "red", "30.00deg 71.00deg 90.00%"],
  ]) {
    demo.scroll(top);
    const modelBounds = demo.viewer.getBoundingClientRect();
    assert.ok(modelBounds.top < demo.browser.innerHeight && modelBounds.top + modelBounds.height > 0);
    assert.equal(demo.viewer.dataset.signal, signal);
    assert.equal(demo.viewer.cameraOrbit, orbit);
    assert.ok(demo.materials[lensIndices[["green", "yellow", "red"].indexOf(signal)]].strength > 0);
  }
  demo.controls[0].dispatchEvent(new Event("click"));
  demo.scroll(-80);
  assert.equal(demo.viewer.dataset.signal, "green", "manual controls persist on mobile too");
  demo.scroll(-90);
  assert.equal(demo.viewer.dataset.signal, "red");
});
