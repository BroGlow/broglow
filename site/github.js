(async () => {
  const badge = document.querySelector(".repository-badge");
  try {
    const repository = new URL(badge.href).pathname;
    const response = await fetch(`https://api.github.com/repos${repository}`);
    if (!response.ok) return;
    const { stargazers_count: stars, forks_count: forks } = await response.json();
    if (![stars, forks].every(count => Number.isSafeInteger(count) && count >= 0)) return;

    badge.querySelector("[data-stars]").textContent = stars.toLocaleString("en-US");
    badge.querySelector("[data-forks]").textContent = forks.toLocaleString("en-US");
    badge.querySelector(".repository-description").hidden = true;
    badge.querySelector(".repository-stats").hidden = false;
  } catch {
    // Keep the source link when GitHub is unreachable or rate-limits the request.
  }
})();
