const resetDialog = document.getElementById("reset-password-dialog");
const resetForm = document.getElementById("reset-password-form");
const resetUserLabel = document.getElementById("reset-password-user");

if (resetDialog instanceof HTMLDialogElement && resetForm instanceof HTMLFormElement) {
  document.querySelectorAll("[data-reset-user]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const uid = btn.getAttribute("data-reset-user");
      if (!uid) return;
      resetForm.action = `/api/users/${encodeURIComponent(uid)}/reset-password`;
      if (resetUserLabel) resetUserLabel.textContent = uid;
      resetForm.reset();
      resetDialog.showModal();
    });
  });

  resetDialog.querySelectorAll("[data-close-dialog]").forEach((el) => {
    el.addEventListener("click", () => resetDialog.close());
  });
}
