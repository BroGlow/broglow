// Runs before paint: applies the saved theme (auto/light/dark) without a flash.
(() => {
  const root = document.documentElement;
  const media = window.matchMedia("(prefers-color-scheme: light)");
  const supported = (value) => value === "auto" || value === "light" || value === "dark";
  let saved;
  try {
    saved = localStorage.getItem("broglow-theme");
  } catch {
    // Storage may be unavailable in private browsing or when opening a local file.
  }
  let choice = supported(saved) ? saved : "auto";

  function apply() {
    root.dataset.themeChoice = choice;
    root.dataset.theme = choice === "auto" ? (media.matches ? "light" : "dark") : choice;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.content = root.dataset.theme === "light" ? "#f5f3ec" : "#181a19";
  }
  apply();
  media.addEventListener("change", apply);

  document.addEventListener("DOMContentLoaded", () => {
    const picker = document.querySelector("#theme");
    picker.value = choice;
    picker.addEventListener("change", () => {
      choice = picker.value;
      apply();
      try {
        localStorage.setItem("broglow-theme", choice);
      } catch {
        // Switching still works for this page when preferences cannot be saved.
      }
    });
  });
})();
