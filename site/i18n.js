(() => {
  const picker = document.querySelector("#language");
  const translations = [];
  // Docs pages read ?lang, so the chosen language survives when storage is unavailable.
  const docLinks = [...document.querySelectorAll('a[href^="docs/"]')]
    .map((link) => [link, link.getAttribute("href")]);

  // English is the HTML fallback; Russian lives next to each translated value.
  for (const attribute of [null, "content", "alt", "aria-label"]) {
    const key = attribute ? `data-ru-${attribute}` : "data-ru";
    for (const element of document.querySelectorAll(`[${key}]`)) {
      translations.push({
        element,
        attribute,
        en: attribute ? element.getAttribute(attribute) : element.textContent,
        ru: element.getAttribute(key),
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
  setLanguage(saved === "ru" || saved === "en" ? saved : "en");
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
