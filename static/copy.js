document.querySelectorAll("[data-copy-target]").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const id = btn.getAttribute("data-copy-target");
    const el = id ? document.getElementById(id) : null;
    if (!el || !(el instanceof HTMLInputElement)) return;

    const value = el.value;
    try {
      await navigator.clipboard.writeText(value);
    } catch {
      el.select();
      document.execCommand("copy");
    }

    const label = btn.textContent ?? "Copy";
    btn.textContent = "Copied!";
    btn.setAttribute("aria-live", "polite");
    window.setTimeout(() => {
      btn.textContent = label;
    }, 2000);
  });
});
