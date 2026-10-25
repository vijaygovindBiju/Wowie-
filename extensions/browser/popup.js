/**
 * Wowie Focus Companion - Popup Controller
 */

document.addEventListener("DOMContentLoaded", async () => {
  const originDisplay = document.getElementById("origin-display");
  const statusBadge = document.getElementById("status-badge");
  const errorMessage = document.getElementById("error-message");
  const reportingToggle = document.getElementById("reporting-toggle");

  // Load preferences and state
  const data = await chrome.storage.local.get({
    reportingEnabled: true,
    hostStatus: "connected",
    currentOrigin: "No active origin",
    lastError: null,
  });

  reportingToggle.checked = data.reportingEnabled;
  updateUI(data);

  // Toggle switch listener
  reportingToggle.addEventListener("change", async (e) => {
    const isEnabled = e.target.checked;
    await chrome.storage.local.set({ reportingEnabled: isEnabled });
    if (!isEnabled) {
      updateUI({
        hostStatus: "paused",
        currentOrigin: "(Reporting Paused)",
        lastError: null,
      });
    } else {
      updateUI({
        hostStatus: "connected",
        currentOrigin: "Scanning...",
        lastError: null,
      });
    }
  });

  // Listen for background state updates
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local") {
      chrome.storage.local.get(
        ["reportingEnabled", "hostStatus", "currentOrigin", "lastError"],
        (items) => {
          updateUI(items);
        }
      );
    }
  });

  function updateUI(state) {
    if (state.currentOrigin) {
      originDisplay.textContent = state.currentOrigin;
    }

    if (state.lastError) {
      errorMessage.textContent = state.lastError;
      errorMessage.classList.remove("hidden");
    } else {
      errorMessage.textContent = "";
      errorMessage.classList.add("hidden");
    }

    statusBadge.className = "badge";
    if (state.hostStatus === "connected") {
      statusBadge.textContent = "Connected";
      statusBadge.classList.add("badge-connected");
    } else if (state.hostStatus === "paused") {
      statusBadge.textContent = "Paused";
      statusBadge.classList.add("badge-paused");
    } else {
      statusBadge.textContent = "Offline";
      statusBadge.classList.add("badge-offline");
    }
  }
});
