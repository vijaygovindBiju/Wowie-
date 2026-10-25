/**
 * Wowie Focus Companion - Background Service Worker
 *
 * Connects to the local native messaging host (`org.wowie.browser_host`)
 * to report active tab origins while strictly stripping paths, queries, and fragments.
 */

const NATIVE_HOST_NAME = "org.wowie.browser_host";

// Debounce state to avoid flooding IPC during rapid tab switching
let lastSentOrigin = null;
let lastSentTime = 0;
const MIN_REPORT_INTERVAL_MS = 500;

/**
 * Strips path, query, hash, and userinfo from a raw URL.
 * Returns null if the URL is not standard HTTP/HTTPS or is private.
 */
function extractSafeOrigin(rawUrl) {
  if (!rawUrl) return null;

  try {
    const url = new URL(rawUrl);
    // Strict scheme allowlist: only http: and https:
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return null;
    }
    // Origin is scheme + host + optional port (e.g. "https://docs.rs")
    return url.origin;
  } catch (_err) {
    return null;
  }
}

/**
 * Queries the active tab in the currently focused window and transmits the origin.
 */
async function reportActiveTab() {
  try {
    const prefs = await chrome.storage.local.get({ reportingEnabled: true });
    if (!prefs.reportingEnabled) {
      await chrome.storage.local.set({
        hostStatus: "paused",
        currentOrigin: "(Reporting Paused)",
      });
      return;
    }

    const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
    if (!tabs || tabs.length === 0) {
      return;
    }

    const activeTab = tabs[0];

    // Privacy rule 1: Never observe incognito/private browsing tabs
    if (activeTab.incognito) {
      await chrome.storage.local.set({
        hostStatus: "connected",
        currentOrigin: "(Private Window - Redacted)",
      });
      return;
    }

    const origin = extractSafeOrigin(activeTab.url);
    if (!origin) {
      await chrome.storage.local.set({
        hostStatus: "connected",
        currentOrigin: "(Internal Browser Page)",
      });
      return;
    }

    const now = Date.now();
    if (origin === lastSentOrigin && now - lastSentTime < MIN_REPORT_INTERVAL_MS) {
      return;
    }

    lastSentOrigin = origin;
    lastSentTime = now;

    const payload = {
      origin: origin,
      is_active_window: true,
      tab_id: activeTab.id || null,
      title: activeTab.title ? activeTab.title.slice(0, 100) : null,
    };

    // Send native message to Wowie desktop companion host
    chrome.runtime.sendNativeMessage(NATIVE_HOST_NAME, payload, (response) => {
      if (chrome.runtime.lastError) {
        const errorMsg = chrome.runtime.lastError.message || "Host connection failed";
        chrome.storage.local.set({
          hostStatus: "error",
          lastError: errorMsg,
          currentOrigin: origin,
        });
      } else {
        chrome.storage.local.set({
          hostStatus: "connected",
          currentOrigin: origin,
          lastError: null,
          lastReportedAt: Date.now(),
        });
      }
    });
  } catch (err) {
    console.error("[Wowie Extension] Error checking active tab:", err);
  }
}

// Tab lifecycle events
chrome.tabs.onActivated.addListener(() => {
  reportActiveTab();
});

chrome.tabs.onUpdated.addListener((_tabId, changeInfo, tab) => {
  if (tab.active && (changeInfo.url || changeInfo.status === "complete")) {
    reportActiveTab();
  }
});

chrome.windows.onFocusChanged.addListener((windowId) => {
  if (windowId === chrome.windows.WINDOW_ID_NONE) {
    // Browser lost desktop focus
    chrome.runtime.sendNativeMessage(
      NATIVE_HOST_NAME,
      {
        origin: lastSentOrigin || "about:blank",
        is_active_window: false,
        tab_id: null,
        title: null,
      },
      () => {
        // Discard result
        if (chrome.runtime.lastError) {
          /* no-op */
        }
      }
    );
  } else {
    reportActiveTab();
  }
});

// Periodic heartbeat to refresh cache freshness
chrome.alarms.create("wowie-heartbeat", { periodInMinutes: 0.1 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "wowie-heartbeat") {
    reportActiveTab();
  }
});

// Initial invocation on load
reportActiveTab();
