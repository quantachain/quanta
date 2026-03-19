/**
 * Quanta Wallet — background.js (MV3 Service Worker)
 *
 * Handles: auto-lock timer, session state (balance cache),
 *          cross-popup message passing.
 *
 * NOTE: Service workers cannot use WASM directly (MV3 restriction).
 * All crypto (Falcon-512) runs in popup.js. Background only manages
 * timing and lightweight state.
 */

const LOCK_AFTER_MS = 5 * 60 * 1000; // Auto-lock after 5 minutes inactivity

// ── Auto-lock alarm ──────────────────────────────────────────────────────────

chrome.runtime.onInstalled.addListener(() => {
  console.log('[Quanta Background] Extension installed');
  scheduleAutoLock();
});

function scheduleAutoLock() {
  chrome.alarms.clear('autolock', () => {
    chrome.alarms.create('autolock', { delayInMinutes: 5 });
  });
}

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === 'autolock') {
    // Signal popup to lock (popup reads this on open)
    chrome.storage.session.set({ locked: true });
    console.log('[Quanta Background] Auto-locked after inactivity');
  }
});

// Reset lock timer on any user activity message from popup
chrome.runtime.onMessage.addListener((msg, sender, respond) => {
  if (msg.type === 'USER_ACTIVITY') {
    scheduleAutoLock();
    chrome.storage.session.set({ locked: false });
    respond({ ok: true });
  }
  if (msg.type === 'GET_LOCK_STATE') {
    chrome.storage.session.get(['locked'], (result) => {
      respond({ locked: result.locked ?? false });
    });
    return true; // async response
  }
});
