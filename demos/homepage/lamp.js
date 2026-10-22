const viewer = document.querySelector("#lamp-model");

if (viewer) {
  viewer.inert = true;
  const demo = document.querySelector("#model-demo");
  const steps = demo ? [...demo.querySelectorAll("[data-model-step]")] : [];
  const controls = demo ? [...demo.querySelectorAll("[data-demo-signal]")] : [];
  const brightnessInput = document.querySelector("#lamp-brightness");
  const brightnessOutput = document.querySelector("#brightness-value");
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  // These indices match the physical green, yellow, and red LED nodes in broglow.glb.
  const lenses = [
    { signal: "green", index: 4, color: [0.08, 1, 0.02] },
    { signal: "yellow", index: 5, color: [1, 0.65, 0.02] },
    { signal: "red", index: 6, color: [1, 0.03, 0.01] },
  ];
  const signals = ["green", "yellow", "red", "off"];
  let signal = signals.includes(document.body.dataset.signal) ? document.body.dataset.signal : "green";
  let brightness = brightnessInput ? Number(brightnessInput.value) / 100 : 0.75;
  let manualTop = null;
  let lastOrbit;
  let frame;
  let visible = false;
  let arActive = false;
  window.addEventListener("broglow:ar-active", event => { arActive = event.detail?.active === true; });

  function setState(state) {
    viewer.inert = state !== 'modelReady';
    for (const host of [viewer, document.body]) {
      delete host.dataset.modelReady;
      delete host.dataset.modelError;
      host.dataset[state] = "true";
    }
    window.dispatchEvent(new CustomEvent(
      state === "modelReady" ? "broglow:model-ready" : "broglow:model-error",
    ));
  }

  function updateSignal() {
    if (viewer.model) {
      for (const lens of lenses) {
        const material = viewer.model.materials[lens.index];
        const active = lens.signal === signal;
        material.pbrMetallicRoughness.setBaseColorFactor([
          ...lens.color.map(channel => channel * (active ? 1 : 0.035)), 1,
        ]);
        material.setEmissiveFactor(active ? lens.color : [0, 0, 0]);
        material.setEmissiveStrength(active ? 1.2 * (brightness / 0.75) : 0);
      }
    }
    viewer.dataset.signal = signal;
    if (demo) demo.dataset.signal = signal;
    for (const control of controls) {
      const active = control.dataset.demoSignal === signal;
      control.classList.toggle("selected", active);
      control.setAttribute("aria-pressed", String(active));
    }
    for (const step of steps) step.classList.toggle("active", step.dataset.modelStep === signal);
    if (brightnessOutput) brightnessOutput.textContent = `${Math.round(brightness * 100)}%`;
    window.dispatchEvent(new CustomEvent("broglow:model-signal", { detail: { signal, brightness } }));
  }

  function setSignal(nextSignal) {
    if (!signals.includes(nextSignal)) return;
    signal = nextSignal;
    updateSignal();
  }

  function setOrbit(orbit) {
    if (reducedMotion.matches || orbit === lastOrbit) return;
    viewer.cameraOrbit = orbit;
    lastOrbit = orbit;
  }

  // The section stays in normal document flow; only the product responds to its position.
  function updateScroll() {
    if (arActive) return;
    const rect = demo.getBoundingClientRect();
    if (manualTop !== null && Math.abs(manualTop - rect.top) < 1) return;
    manualTop = null;
    const target = window.innerHeight * 0.5;
    const compact = window.innerWidth <= 900;
    const modelBounds = compact ? viewer.getBoundingClientRect() : null;
    const positions = steps.map((step, index) => {
      // Stacked layouts show every state while the model itself crosses the viewport.
      if (compact) return modelBounds.top + modelBounds.height / 2
        + (index / Math.max(1, steps.length - 1) - 0.5) * window.innerHeight * 0.5;
      const bounds = step.getBoundingClientRect();
      return bounds.top + bounds.height / 2;
    });
    let nearest = 0;
    for (let index = 1; index < positions.length; index++) {
      if (Math.abs(positions[index] - target) < Math.abs(positions[nearest] - target)) nearest = index;
    }
    const nextSignal = steps[nearest].dataset.modelStep;
    if (nextSignal !== signal) setSignal(nextSignal);
    let left = 0;
    while (left < positions.length - 2 && positions[left + 1] < target) left++;
    const right = Math.min(left + 1, positions.length - 1);
    const progress = Math.max(0, Math.min(1, (target - positions[left]) / Math.max(1, positions[right] - positions[left])));
    const orbit = index => (steps[index].dataset.orbit || "-22deg 77deg 100%").split(/\s+/).map(parseFloat);
    const start = orbit(left);
    const end = orbit(right);
    const values = start.map((value, index) => (value + (end[index] - value) * progress).toFixed(2));
    setOrbit(`${values[0]}deg ${values[1]}deg ${values[2]}%`);
  }

  function tick() {
    frame = undefined;
    if (!visible) return;
    updateScroll();
    frame = requestAnimationFrame(tick);
  }

  if (demo && steps.length) {
    const observer = new IntersectionObserver(entries => {
      visible = entries[0].isIntersecting;
      if (visible && frame === undefined) frame = requestAnimationFrame(tick);
      else if (!visible && frame !== undefined) { cancelAnimationFrame(frame); frame = undefined; }
    });
    observer.observe(demo);
    for (const control of controls) {
      control.addEventListener("click", () => {
        manualTop = demo.getBoundingClientRect().top;
        setSignal(control.dataset.demoSignal);
        const step = steps.find(item => item.dataset.modelStep === signal);
        if (step?.dataset.orbit) setOrbit(step.dataset.orbit);
      });
    }
  } else {
    window.addEventListener("broglow:signal", event => setSignal(event.detail?.signal));
    window.addEventListener("broglow:camera", event => {
      if (typeof event.detail?.orbit === "string") setOrbit(event.detail.orbit);
    });
  }

  brightnessInput?.addEventListener("input", () => {
    const value = Number(brightnessInput.value);
    if (!Number.isFinite(value)) return;
    brightness = Math.max(0, Math.min(1, value / 100));
    updateSignal();
  });

  function respectMotionPreference() {
    viewer.interpolationDecay = reducedMotion.matches ? 0 : 100;
    if (reducedMotion.matches) viewer.removeAttribute("auto-rotate");
  }
  respectMotionPreference();
  reducedMotion.addEventListener("change", respectMotionPreference);

  viewer.addEventListener("load", () => {
    try {
      viewer.model.materials[0].pbrMetallicRoughness.setBaseColorFactor([0.8, 0.095, 0.005, 1]);
      viewer.model.materials[1].pbrMetallicRoughness.setBaseColorFactor([0.006, 0.007, 0.006, 1]);
      updateSignal();
      setState("modelReady");
    } catch {
      setState("modelError");
    }
  });
  viewer.addEventListener("error", () => setState("modelError"));

  import("../../site/vendor/model-viewer-module-4.3.1.min.js")
    .catch(() => setState("modelError"));
}
