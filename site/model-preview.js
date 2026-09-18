(() => {
  const viewer = document.querySelector("#lamp-model");
  const poster = document.querySelector("#model-poster");
  const load = document.querySelector("#model-load");
  const reset = document.querySelector("#model-reset");
  const loading = document.querySelector("#model-loading");
  const error = document.querySelector("#model-error");
  const help = document.querySelector("#model-help");
  const initialOrbit = viewer.getAttribute("camera-orbit");
  let failures = 0;
  // Browsers and model-viewer cache failed loads; a user retry needs a fresh URL.
  const retryUrl = (url) => failures ? `${url}?retry=${failures}` : url;

  function focusModel() {
    // model-viewer 4.3.1 keeps its keyboard camera target inside the open shadow DOM.
    viewer.shadowRoot?.querySelector('[role="img"][tabindex="0"]')?.focus();
  }

  function fail() {
    failures += 1;
    viewer.hidden = true;
    viewer.removeAttribute("src");
    poster.hidden = false;
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
  viewer.addEventListener("poster-dismissed", focusModel);
  viewer.addEventListener("load", () => {
    poster.hidden = true;
    load.hidden = true;
    loading.hidden = true;
    reset.hidden = false;
    help.hidden = false;
  });
  reset.addEventListener("click", () => {
    viewer.cameraOrbit = initialOrbit;
    viewer.cameraTarget = "auto auto auto";
    viewer.jumpCameraToGoal();
    focusModel();
  });
  load.addEventListener("click", () => {
    load.disabled = true;
    loading.hidden = false;
    error.hidden = true;
    if (customElements.get("model-viewer")) {
      showModel();
      return;
    }
    const script = document.createElement("script");
    script.type = "module";
    script.src = retryUrl(load.dataset.viewerSrc);
    script.addEventListener("load", showModel, { once: true });
    script.addEventListener("error", fail, { once: true });
    document.head.append(script);
  });
  load.hidden = false;
})();
