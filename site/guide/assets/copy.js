// Keep Material's buttons; use the Clipboard API instead of legacy execCommand.
document.addEventListener("click", async (event) => {
  const button = event.target.closest('[data-md-type="copy"]');
  if (!button) return;
  event.preventDefault();
  event.stopImmediatePropagation();
  const code = document.querySelector(button.dataset.clipboardTarget);
  let feedback = code.closest("pre").nextElementSibling;
  if (!feedback?.classList.contains("copy-feedback")) {
    feedback = document.createElement("span");
    feedback.className = "copy-feedback";
    feedback.setAttribute("role", "status");
    code.closest("pre").after(feedback);
  }
  const ru = document.documentElement.lang === "ru";
  button.disabled = true;
  try {
    await navigator.clipboard.writeText(code.textContent.trim());
    feedback.textContent = ru ? "Скопировано!" : "Copied!";
  } catch {
    feedback.textContent = ru
      ? "Не удалось скопировать. Выделите текст и скопируйте вручную."
      : "Couldn’t copy. Select the text and copy it manually.";
  } finally {
    button.disabled = false;
  }
}, true);
