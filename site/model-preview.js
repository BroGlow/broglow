import { PointLight } from "three";

(() => {
  const viewer = document.querySelector("#lamp-model");
  const composer = viewer.querySelector("effect-composer");
  const signalLight = new PointLight(0xffffff, 0, 0.035, 2);
  const poster = document.querySelector("#model-poster");
  const load = document.querySelector("#model-load");
  const reset = document.querySelector("#model-reset");
  const loading = document.querySelector("#model-loading");
  const error = document.querySelector("#model-error");
  const help = document.querySelector("#model-help");
  const space = document.querySelector("#model-space");
  const viewToggle = document.querySelector("#view-toggle");
  const viewModel = document.querySelector("#view-model");
  const viewPhoto = document.querySelector("#view-photo");
  const dialog = document.querySelector("#ar-dialog");
  const qr = document.querySelector("#ar-qr");
  const link = document.querySelector("#ar-link");
  const local = document.querySelector("#ar-local");
  const qrError = document.querySelector("#ar-qr-error");
  const signals = document.querySelector(".signal-demo");
  const assembly = document.querySelector("#assembly-model");
  const assemblyLoading = document.querySelector("#assembly-loading");
  const assemblyError = document.querySelector("#assembly-error");
  const assemblyRetry = document.querySelector("#assembly-retry");
  const assemblyReset = document.querySelector("#assembly-reset");
  const assemblyOrbit = assembly.getAttribute("camera-orbit");
  const assemblySrc = assembly.getAttribute("src");
  let assemblyFailures = 0;
  function failAssembly() {
    assemblyLoading.hidden = true;
    assemblyError.hidden = false;
    assemblyRetry.hidden = false;
    assemblyReset.hidden = true;
  }
  assemblyLoading.hidden = false;
  assembly.addEventListener("error", failAssembly);
  assembly.addEventListener("load", () => {
    assemblyLoading.hidden = assemblyError.hidden = assemblyRetry.hidden = true;
    assemblyReset.hidden = false;
  });
  assemblyRetry.addEventListener("click", () => {
    assemblyError.hidden = assemblyRetry.hidden = true;
    assemblyLoading.hidden = false;
    assembly.src = `${assemblySrc}?retry=${++assemblyFailures}`;
    loadModel();
  });
  assemblyReset.addEventListener("click", () => {
    assembly.cameraOrbit = assemblyOrbit;
    assembly.cameraTarget = "auto auto auto";
    assembly.jumpCameraToGoal();
    assembly.shadowRoot?.querySelector('[role="img"][tabindex="0"]')?.focus();
  });
  // Lens material indices in broglow.glb; the regression test checks the LED nodes.
  const lenses = [
    { mode: "working", index: 4, color: [0.08, 1, 0.02] },
    { mode: "idle", index: 5, color: [1, 0.65, 0.02] },
    { mode: "needs-you", index: 6, color: [1, 0.03, 0.01] },
  ];
  const initialOrbit = viewer.getAttribute("camera-orbit");
  let failures = 0;
  let focusOnLoad = false;
  // Browsers and model-viewer cache failed loads; a user retry needs a fresh URL.
  const retryUrl = (url) => failures ? `${url}?retry=${failures}` : url;

  function updateSignal() {
    if (!viewer.model || viewer.hidden) return;
    const selected = signals.querySelector("input:checked").value;
    signalLight.intensity = 0;
    for (const { mode, index, color } of lenses) {
      const material = viewer.model.materials[index];
      const active = mode === selected;
      material.pbrMetallicRoughness.setBaseColorFactor([
        ...color.map(value => value * (active ? 1 : 0.08)), 1,
      ]);
      material.setEmissiveFactor(active ? color : [0, 0, 0]);
      material.setEmissiveStrength(active ? 0.8 : 0);
      if (active) {
        const mesh = [...(composer.selection ?? [])].find(mesh => mesh.name === `REFERENCE_ONLY_LED_${index - 4}`);
        if (mesh) {
          mesh.geometry.computeBoundingBox();
          const bounds = mesh.geometry.boundingBox;
          bounds.getCenter(signalLight.position);
          // Place the source inside the dome so it does not overexpose its own front.
          signalLight.position.z = bounds.max.z - (bounds.max.x - bounds.min.x) / 4;
          mesh.add(signalLight);
          signalLight.color.fromArray(color);
          signalLight.intensity = 0.05; // Tuned for the metre-scale model and renderer falloff.
        }
      }
    }
    composer.queueRender?.();
  }

  function initializePreview() {
    const materials = viewer.model.materials;
    materials[0].pbrMetallicRoughness.setBaseColorFactor([0.8, 0.095, 0.005, 1]);
    materials[1].pbrMetallicRoughness.setBaseColorFactor([0.006, 0.007, 0.006, 1]);
    updateSignal();
  }

  function showPhoto() {
    poster.src = signals.querySelector("input:checked").dataset.photo;
  }

  function setView(photo) {
    viewPhoto.ariaPressed = String(photo);
    viewModel.ariaPressed = String(!photo);
    // Keep the viewer laid out: display:none would zero its size and break the camera framing.
    viewer.dataset.view = photo ? "photo" : "model";
    poster.hidden = !photo;
    reset.hidden = help.hidden = photo;
    if (photo) showPhoto();
    else updateSignal();
  }

  function focusModel() {
    // model-viewer 4.3.1 keeps its keyboard camera target inside the open shadow DOM.
    viewer.shadowRoot?.querySelector('[role="img"][tabindex="0"]')?.focus();
  }

  function fail() {
    failures += 1;
    viewer.hidden = true;
    viewer.removeAttribute("src");
    poster.hidden = false;
    showPhoto();
    viewToggle.hidden = true;
    load.hidden = false;
    load.disabled = false;
    loading.hidden = true;
    error.hidden = false;
    reset.hidden = true;
    help.hidden = true;
  }

  function showModel() {
    viewer.hidden = false;
    viewer.src = retryUrl(viewer.dataset.modelSrc);
  }

  viewer.addEventListener("error", fail);
  composer.addEventListener("updated-selection", updateSignal);
  viewer.addEventListener("before-render", initializePreview);
  viewer.addEventListener("poster-dismissed", () => {
    if (focusOnLoad && !signals.contains(document.activeElement)) focusModel();
    focusOnLoad = false;
  });
  viewer.addEventListener("load", () => {
    initializePreview();
    load.hidden = loading.hidden = true;
    viewToggle.hidden = false;
    setView(false);
  });
  viewPhoto.addEventListener("click", () => setView(true));
  viewModel.addEventListener("click", () => setView(false));
  reset.addEventListener("click", () => {
    viewer.cameraOrbit = initialOrbit;
    viewer.cameraTarget = "auto auto auto";
    viewer.jumpCameraToGoal();
    focusModel();
  });
  function loadModel() {
    if (load.disabled || !viewer.hidden) return;
    load.disabled = true;
    load.hidden = true;
    loading.hidden = false;
    error.hidden = true;
    if (customElements.get("model-viewer") && customElements.get("effect-composer")) {
      showModel();
      return;
    }
    const script = document.createElement("script");
    script.type = "module";
    script.src = retryUrl(load.dataset.viewerSrc);
    script.addEventListener("load", showModel, { once: true });
    script.addEventListener("error", () => {
      fail();
      if (!customElements.get("model-viewer")) failAssembly();
    }, { once: true });
    document.head.append(script);
  }
  load.addEventListener("click", () => {
    focusOnLoad = true;
    loadModel();
  });
  // Click also handles the initially checked Working option; change covers keyboard use.
  for (const event of ["click", "change"]) signals.addEventListener(event, (event) => {
    if (!event.target.matches('input[name="signal"]')) return;
    loadModel();
    updateSignal();
    if (!poster.hidden) showPhoto();
  });
  space.addEventListener("click", () => {
    const url = new URL(location.href);
    url.search = "";
    url.hash = "view-in-space";
    link.href = url.href;
    local.hidden = /^https?:$/.test(url.protocol) &&
      !/^(localhost|.*\.localhost|127\..*|\[::1\]|0\.0\.0\.0)$/.test(url.hostname);
    qr.hidden = true;
    qrError.hidden = true;
    if (local.hidden) {
      try {
        const code = qrcode(0, "M");
        code.addData(url.href);
        code.make();
        qr.src = code.createDataURL(6, 24);
        qr.hidden = false;
      } catch {
        qrError.hidden = false;
      }
    }
    dialog.showModal();
  });
  dialog.addEventListener("click", (event) => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right ||
        event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
  });
  link.addEventListener("click", (event) => {
    event.preventDefault();
    dialog.close();
    focusOnLoad = true;
    loadModel();
  });
  load.hidden = false;
  space.hidden = false;
  loadModel();
})();
