(() => {
  const links = [...document.querySelectorAll("a[hreflang]")];
  const supported = (value) => value === "ru" || value === "en";
  const explicit = new URL(location.href).searchParams.get("lang");
  let saved;
  try {
    saved = localStorage.getItem("clawlight-language");
    if (supported(explicit)) localStorage.setItem("clawlight-language", explicit);
  } catch {
    // The explicit URL choice still works when storage is unavailable.
  }
  const browser = [...(navigator.languages || []), navigator.language]
    .filter(Boolean).map((locale) => locale.toLowerCase().split("-")[0]).find(supported);
  const preferred = supported(explicit) ? explicit : supported(saved) ? saved : browser || "en";
  const target = links.find((link) => link.hreflang === preferred);
  if (document.documentElement.lang !== preferred && target) {
    const url = new URL(target.href);
    url.hash = location.hash;
    if (supported(explicit)) url.searchParams.set("lang", explicit);
    location.replace(url.href);
    return;
  }
  for (const link of links) {
    link.addEventListener("click", (event) => {
      try { localStorage.setItem("clawlight-language", link.hreflang); } catch {}
      const url = new URL(link.href);
      url.hash = location.hash;
      link.href = url.href;
      if (!event.button && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey) {
        event.preventDefault();
        event.stopImmediatePropagation();
        location.assign(url.href);
      }
    });
    const url = new URL(link.href);
    url.searchParams.set("lang", link.hreflang);
    url.hash = location.hash;
    link.href = url.href;
  }
})();
