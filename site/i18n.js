(() => {
  const picker = document.querySelector("#language");
  const translations = [];
  // Docs pages read ?lang, so the chosen language survives when storage is unavailable.
  const docLinks = [...document.querySelectorAll('a[href^="docs/"]')]
    .map((link) => [link, link.getAttribute("href")]);

  // Russian is the HTML fallback; English lives next to each translated value.
  for (const attribute of [null, "content", "alt", "aria-label"]) {
    const key = attribute ? `data-en-${attribute}` : "data-en";
    for (const element of document.querySelectorAll(`[${key}]`)) {
      translations.push({
        element,
        attribute,
        ru: attribute ? element.getAttribute(attribute) : element.textContent,
        en: element.getAttribute(key),
      });
    }
  }

  function setLanguage(language) {
    for (const entry of translations) {
      if (entry.attribute) entry.element.setAttribute(entry.attribute, entry[language]);
      else entry.element.textContent = entry[language];
    }
    for (const [link, href] of docLinks) {
      const [path, hash] = href.split("#");
      link.setAttribute("href", `${path}?lang=${language}${hash === undefined ? "" : `#${hash}`}`);
    }
    document.documentElement.lang = language;
    picker.value = language;
  }

  let saved;
  try {
    saved = localStorage.getItem("broglow-language");
  } catch {
    // Storage may be unavailable in private browsing or when opening a local file.
  }
  const browserLanguage = [...(navigator.languages || []), navigator.language]
    .filter(Boolean)
    .map((locale) => locale.toLowerCase().split("-")[0])
    .find((language) => language === "ru" || language === "en");
  setLanguage(saved === "ru" || saved === "en" ? saved : browserLanguage || "en");
  picker.hidden = false;

  picker.addEventListener("change", () => {
    setLanguage(picker.value);
    try {
      localStorage.setItem("broglow-language", picker.value);
    } catch {
      // Switching still works for this page when preferences cannot be saved.
    }
  });

})();
