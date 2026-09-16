(() => {
  const picker = document.querySelector("#language");
  const translations = [];

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
    document.documentElement.lang = language;
    picker.value = language;
  }

  let saved;
  try {
    saved = localStorage.getItem("clawlight-language");
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
      localStorage.setItem("clawlight-language", picker.value);
    } catch {
      // Switching still works for this page when preferences cannot be saved.
    }
  });
})();
