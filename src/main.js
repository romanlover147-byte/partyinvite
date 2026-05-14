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

function createGoogleCalendarUrl() {
  const start = new Date();
  start.setDate(start.getDate() + 7);
  start.setHours(19, 0, 0, 0);

  const end = new Date(start);
  end.setHours(22, 0, 0, 0);

  const toUtcString = (date) =>
    date.toISOString().replace(/[-:]/g, "").split(".")[0] + "Z";

  const params = new URLSearchParams({
    action: "TEMPLATE",
    text: "Party Invitation",
    details: "You accepted the invitation. See you at the party!",
    location: "Party Venue",
    dates: `${toUtcString(start)}/${toUtcString(end)}`,
  });

  return `https://calendar.google.com/calendar/render?${params.toString()}`;
}

async function runSystemPreflight() {
  const statusEl = document.querySelector("#system-status");
  if (!statusEl) return;

  try {
    const clientDate = formatIsoDateLocal(new Date());
    const result = await invoke("system_preflight", { clientDate });

    if (result.calendarMatchesRuntime) {
      statusEl.textContent = `Everything is set for your invite • ${result.runtimeDate}`;
      statusEl.style.color = "#9fe39f";
      return;
    }

    statusEl.textContent = `Quick calendar check needed • ${result.runtimeDate}`;
    statusEl.style.color = "#ffd39f";
  } catch (error) {
    statusEl.textContent = "We could not check invite timing right now.";
    statusEl.style.color = "#ffb1b1";
    console.error(error);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  const dateEl = document.querySelector("#event-date");
  if (dateEl) dateEl.textContent = formatInviteDate(new Date());
  
  await runSystemPreflight();

  const msgEl = document.querySelector("#rsvp-msg");
  const btnAccept = document.querySelector("#btn-accept");
  const btnDecline = document.querySelector("#btn-decline");

  btnAccept.addEventListener("click", () => {
    msgEl.textContent = "🎉 RSVP confirmed! Opening your party reminder.";
    msgEl.style.color = "#9fe39f";
    msgEl.style.fontWeight = "600";

    const calendarUrl = createGoogleCalendarUrl();
    const calendarWindow = window.open(calendarUrl, "_blank");
    if (!calendarWindow) {
      msgEl.textContent = "🎉 RSVP confirmed! Add a reminder for next week at 7:00 PM.";
    }

    btnAccept.disabled = true;
    btnDecline.disabled = true;
  });

  btnDecline.addEventListener("click", () => {
    msgEl.textContent = "We'll miss you — perhaps next time.";
    msgEl.style.color = "rgba(255,255,255,0.55)";
  });
});
