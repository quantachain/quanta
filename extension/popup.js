/**
 * Quanta Wallet Extension — popup.js
 *
 * MV3 CSP-compliant (no inline onclick).
 */

'use strict';

const STORAGE_KEY = 'quanta_wallet_v1';
const SETTINGS_KEY = 'quanta_settings_v1';
const MICROUNITS = 1_000_000;

let state = {
  publicKey: null,
  secretKey: null,
  address: null,
  balance: 0,
  txHistory: [],
  mnemonic: null,
  settings: {
    rpc_url: 'http://localhost:3000',
    explorer_url: 'https://explorer.quantachain.org',
    network: 'testnet',
  },
};

let wasm = null;

function escapeHtml(unsafe) {
  if (!unsafe) return '';
  return String(unsafe).replace(/[&<"'>]/g, function (m) {
    return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#039;' }[m];
  });
}

// ── Storage helpers (chrome.storage.local) ───────────────────────────────────

function storageGet(key) {
  return new Promise(resolve => chrome.storage.local.get([key], r => resolve(r[key])));
}

function storageSet(key, value) {
  return new Promise(resolve => chrome.storage.local.set({ [key]: value }, resolve));
}

function storageRemove(key) {
  return new Promise(resolve => chrome.storage.local.remove([key], resolve));
}

// ── WASM ─────────────────────────────────────────────────────────────────────

async function loadWasm() {
  try {
    const url = chrome.runtime.getURL('pkg/quanta_wasm.js');
    const module = await import(url);
    const wasmUrl = chrome.runtime.getURL('pkg/quanta_wasm_bg.wasm');
    await module.default(wasmUrl);
    wasm = module;
    console.log('[Quanta] WASM loaded');
  } catch (e) {
    console.warn('[Quanta] WASM not loaded:', e.message);
    wasm = null;
  }
}

// ── Activity ping ────────────────────────────────────────────────────────────

function pingActivity() {
  chrome.runtime.sendMessage({ type: 'USER_ACTIVITY' });
}

document.addEventListener('click', pingActivity, { passive: true });
document.addEventListener('keydown', pingActivity, { passive: true });

// ── Screen navigation ─────────────────────────────────────────────────────────

function showScreen(id) {
  document.querySelectorAll('.screen').forEach(s => s.classList.remove('active'));
  const el = document.getElementById(id);
  if (el) { el.classList.add('active'); el.scrollTop = 0; }
  closeAllPanels();
}

function switchTab(id, btn) {
  document.querySelectorAll('.tab-content').forEach(t => t.classList.remove('active'));
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.getElementById(id).classList.add('active');
  btn.classList.add('active');
}

function showPanel(id) {
  // If opening a new panel, hide others unless it's an overlay panel (export over account)
  if (id !== 'export-panel') closeAllPanels();

  const panel = document.getElementById(id);
  if (panel) panel.classList.add('open');
  const overlay = document.getElementById('overlay');
  if (overlay) overlay.classList.remove('hidden');

  if (id === 'receive-panel') renderQr();
}

function closePanel(id) {
  const panel = document.getElementById(id);
  if (panel) panel.classList.remove('open');

  // Custom reset for Export panel
  if (id === 'export-panel') {
    document.getElementById('export-pw-group').classList.remove('hidden');
    document.getElementById('export-key-result').classList.add('hidden');
    document.getElementById('export-password').value = '';
    document.getElementById('export-error').classList.add('hidden');
  }

  // Hide overlay if no panels are open
  if (!document.querySelector('.side-panel.open')) {
    const overlay = document.getElementById('overlay');
    if (overlay) overlay.classList.add('hidden');
  }
}

function closeAllPanels() {
  document.querySelectorAll('.side-panel').forEach(p => p.classList.remove('open'));
  const overlay = document.getElementById('overlay');
  if (overlay) overlay.classList.add('hidden');

  // Reset Export panel
  const epw = document.getElementById('export-pw-group');
  if (epw) epw.classList.remove('hidden');
  const ekr = document.getElementById('export-key-result');
  if (ekr) ekr.classList.add('hidden');
  const epwd = document.getElementById('export-password');
  if (epwd) epwd.value = '';
}

// ── Toast ─────────────────────────────────────────────────────────────────────

let toastTimer = null;
function toast(msg, ms = 3000) {
  const el = document.getElementById('toast');
  if (!el) return;
  el.textContent = msg;
  el.classList.remove('hidden');
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    el.classList.remove('show');
    setTimeout(() => el.classList.add('hidden'), 300);
  }, ms);
}

// ── Create wallet flow ────────────────────────────────────────────────────────

function toggleCreateBtn() {
  const btn = document.getElementById('btn-generate-wallet');
  const chk = document.getElementById('chk-understand');
  if (btn && chk) btn.disabled = !chk.checked;
}

async function createWallet() {
  showScreen('screen-loading');
  const msgEl = document.getElementById('loading-msg');
  if (msgEl) msgEl.textContent = 'Generating Falcon-512 keys…';
  try {
    let mnemonicPhrase, pkHex, skHex, address;
    if (wasm) {
      const result = wasm.generate_wallet();
      mnemonicPhrase = result.mnemonic;
      pkHex = result.public_key;
      skHex = result.secret_key;
      address = result.address;
    } else {
      throw new Error('WASM not loaded');
    }
    state.mnemonic = mnemonicPhrase;
    state.publicKey = pkHex;
    state.secretKey = skHex;
    state.address = address;
    renderMnemonicGrid(mnemonicPhrase);
    showScreen('screen-mnemonic');
  } catch (e) {
    toast('Error: ' + e.message);
    showScreen('screen-create-warn');
  }
}

function renderMnemonicGrid(phrase) {
  const words = phrase.split(' ');
  const grid = document.getElementById('mnemonic-grid');
  if (!grid) return;
  grid.innerHTML = words.map((w, i) => `
    <div class="mnemonic-word">
      <span class="word-num">${i + 1}.</span>
      <span class="word-text">${w}</span>
    </div>`).join('');
}

function copyMnemonic() {
  if (state.mnemonic) {
    navigator.clipboard.writeText(state.mnemonic).then(() => toast('Copied recovery phrase'));
  }
}

// ── Mnemonic confirm ──────────────────────────────────────────────────────────

const confirmPositions = [];

function setupConfirmInputs() {
  if (!state.mnemonic) return;
  const words = state.mnemonic.split(' ');
  confirmPositions.length = 0;
  while (confirmPositions.length < 3) {
    const r = Math.floor(Math.random() * 24);
    if (!confirmPositions.includes(r)) confirmPositions.push(r);
  }
  confirmPositions.sort((a, b) => a - b);
  const grid = document.getElementById('confirm-inputs');
  if (!grid) return;
  grid.innerHTML = confirmPositions.map(i => `
    <div class="confirm-row">
      <span class="confirm-num">Word #${i + 1}</span>
      <input type="text" id="confirm-word-${i}" placeholder="word ${i + 1}" autocomplete="off" spellcheck="false">
    </div>`).join('');
}

function confirmMnemonic() {
  const words = (state.mnemonic || '').split(' ');
  const errEl = document.getElementById('confirm-error');
  const ok = confirmPositions.every(pos => {
    const el = document.getElementById(`confirm-word-${pos}`);
    return el && el.value.trim().toLowerCase() === words[pos];
  });
  if (!ok) { if (errEl) errEl.classList.remove('hidden'); return; }
  if (errEl) errEl.classList.add('hidden');
  showScreen('screen-password');
}

// ── Password ──────────────────────────────────────────────────────────────────

function checkPasswordStrength() {
  const pwEl = document.getElementById('pw-new');
  if (!pwEl) return;
  const pw = pwEl.value;
  const fill = document.getElementById('strength-fill');
  const label = document.getElementById('strength-label');
  let score = 0;
  if (pw.length >= 8) score++;
  if (pw.length >= 12) score++;
  if (/[A-Z]/.test(pw)) score++;
  if (/[0-9]/.test(pw)) score++;
  if (/[^A-Za-z0-9]/.test(pw)) score++;
  const widths = ['0%', '20%', '40%', '65%', '85%', '100%'];
  const colors = ['#ff4d6a', '#ff4d6a', '#ffb830', '#ffb830', '#00ff88', '#00d4ff'];
  const labels = ['', 'Weak', 'Fair', 'Good', 'Strong', 'Very Strong'];
  if (fill) { fill.style.width = widths[score]; fill.style.background = colors[score]; }
  if (label) { label.textContent = labels[score]; label.style.color = colors[score]; }
}

function togglePw(id) {
  const el = document.getElementById(id);
  if (!el) return;
  el.type = el.type === 'password' ? 'text' : 'password';
  const btn = document.querySelector(`button[id*="${id}"]`);
  if (btn) btn.textContent = el.type === 'password' ? 'Show' : 'Hide';
}

async function setPassword() {
  const pw1El = document.getElementById('pw-new');
  const pw2El = document.getElementById('pw-confirm');
  if (!pw1El || !pw2El) return;
  const pw1 = pw1El.value;
  const pw2 = pw2El.value;
  const errEl = document.getElementById('pw-error');
  if (pw1 !== pw2) { if (errEl) errEl.classList.remove('hidden'); return; }
  if (pw1.length < 8) { toast('Min 8 characters'); return; }
  if (errEl) errEl.classList.add('hidden');
  showScreen('screen-loading');
  const msgEl = document.getElementById('loading-msg');
  if (msgEl) msgEl.textContent = 'Encrypting wallet…';
  try {
    await saveWallet(state.secretKey, state.publicKey, state.address, state.mnemonic, pw1);
    state.secretKey = null; state.mnemonic = null;
    await enterMain();
  } catch (e) {
    toast('Error: ' + e.message); showScreen('screen-password');
  }
}

// ── Import ────────────────────────────────────────────────────────────────────

function validateImportPhrase() {
  const phrEl = document.getElementById('import-phrase');
  if (!phrEl) return;
  const phrase = phrEl.value.trim();
  const words = phrase.split(/\s+/).filter(Boolean);
  const valid = words.length === 24 && (wasm ? wasm.validate_mnemonic(phrase) : true);
  const vEl = document.getElementById('import-valid');
  const invEl = document.getElementById('import-invalid');
  const btn = document.getElementById('btn-import-go');
  if (vEl) vEl.classList.toggle('hidden', !valid);
  if (invEl) invEl.classList.toggle('hidden', valid || phrase === '');
  if (btn) btn.disabled = !valid;
}

async function importWallet() {
  const phrEl = document.getElementById('import-phrase');
  const pwdEl = document.getElementById('import-password');
  if (!phrEl || !pwdEl) return;
  const phrase = phrEl.value.trim();
  const passphrase = document.getElementById('import-passphrase')?.value || '';
  const password = pwdEl.value;
  const errEl = document.getElementById('import-error');
  if (errEl) errEl.classList.add('hidden');
  if (password.length < 8) {
    if (errEl) { errEl.textContent = 'Password must be at least 8 characters'; errEl.classList.remove('hidden'); }
    return;
  }
  showScreen('screen-loading');
  const msgEl = document.getElementById('loading-msg');
  if (msgEl) msgEl.textContent = 'Restoring wallet…';
  try {
    if (!wasm) throw new Error('WASM not loaded');
    const result = wasm.import_wallet(phrase, passphrase, 0);
    await saveWallet(result.secret_key, result.public_key, result.address, phrase, password);
    await enterMain();
  } catch (e) {
    if (errEl) { errEl.textContent = 'Error: ' + e.message; errEl.classList.remove('hidden'); }
    showScreen('screen-import');
  }
}

// ── Encrypted storage (Web Crypto AES-GCM + PBKDF2) ─────────────────────────

async function deriveKey(password, salt) {
  const enc = new TextEncoder();
  const base = await crypto.subtle.importKey('raw', enc.encode(password), 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 250_000, hash: 'SHA-256' },
    base,
    { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']
  );
}

async function saveWallet(skHex, pkHex, address, mnemonic, password) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(password, salt);
  const plain = new TextEncoder().encode(JSON.stringify({ skHex, pkHex, address, mnemonic }));
  const cipher = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, plain);
  await storageSet(STORAGE_KEY, {
    salt: Array.from(salt), iv: Array.from(iv),
    data: Array.from(new Uint8Array(cipher)),
    address, pkHex,
  });
}

async function loadWalletData(password) {
  const stored = await storageGet(STORAGE_KEY);
  if (!stored) throw new Error('No wallet found');
  const salt = new Uint8Array(stored.salt);
  const iv = new Uint8Array(stored.iv);
  const data = new Uint8Array(stored.data);
  const key = await deriveKey(password, salt);
  const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, data);
  return JSON.parse(new TextDecoder().decode(plain));
}

async function walletExists() {
  const s = await storageGet(STORAGE_KEY);
  return !!s;
}

async function getPublicInfo() {
  const s = await storageGet(STORAGE_KEY);
  return { address: s?.address || null, pkHex: s?.pkHex || null };
}

// ── Main wallet ───────────────────────────────────────────────────────────────

async function enterMain() {
  const { address, pkHex } = await getPublicInfo();
  state.address = address;
  state.publicKey = pkHex;
  await loadSettings();
  updateMainUI();
  showScreen('screen-main');
  await refreshBalance();
  await loadHistory();

  const params = new URLSearchParams(window.location.search);
  if (params.get('flow')) {
    toast('Wallet Setup Complete! You can now use your wallet.', 3000);
  }
}

function updateMainUI() {
  const a = state.address || '';
  const addrEl = document.getElementById('wallet-address');
  if (addrEl) addrEl.textContent = a ? a.slice(0, 8) + '...' + a.slice(-6) : '';
  const rAddr = document.getElementById('receive-address-text');
  if (rAddr) rAddr.textContent = a;
  const menuAddr = document.getElementById('menu-wallet-address');
  if (menuAddr) menuAddr.textContent = a;

  const balEl = document.getElementById('asset-bal-val');
  if (balEl) balEl.textContent = (state.balance / MICROUNITS).toFixed(6);

  const netBadge = document.getElementById('network-badge');
  if (netBadge) netBadge.textContent = state.settings.network === 'testnet' ? 'Testnet' : 'Mainnet';

  const urlEl = document.getElementById('rpc-url');
  if (urlEl) urlEl.value = state.settings.rpc_url;
  const expEl = document.getElementById('explorer-url');
  if (expEl) expEl.value = state.settings.explorer_url || '';
  const selEl = document.getElementById('network-select');
  if (selEl) selEl.value = state.settings.network;
}

// ── Node API ──────────────────────────────────────────────────────────────────

function rpcUrl(path) {
  return (state.settings.rpc_url || 'http://localhost:3000').replace(/\/$/, '') + path;
}

async function refreshBalance() {
  if (!state.address) return;
  try {
    const r = await fetch(rpcUrl(`/balance/${state.address}`));
    const data = await r.json();
    state.balance = data.balance ?? data.amount ?? 0;
    const b1 = document.getElementById('balance-val');
    const b2 = document.getElementById('asset-bal-val');
    if (b1) b1.textContent = (state.balance / MICROUNITS).toFixed(6);
    if (b2) b2.textContent = (state.balance / MICROUNITS).toFixed(6);
  } catch {
    const b1 = document.getElementById('balance-val');
    if (b1) b1.textContent = 'Offline';
  }
}

async function loadHistory() {
  if (!state.address) return;
  try {
    const r = await fetch(rpcUrl(`/transactions/${state.address}`));
    if (!r.ok) return;
    const data = await r.json();
    state.txHistory = Array.isArray(data) ? data : (data.transactions ?? []);
    renderHistory();
  } catch { }
}

function renderHistory() {
  const list = document.getElementById('tx-list');
  if (!list) return;
  if (!state.txHistory.length) {
    list.innerHTML = '<div class="tx-empty">No transactions yet</div>'; return;
  }
  list.innerHTML = state.txHistory.slice(0, 30).map(tx => {
    const out = tx.sender?.toLowerCase() === state.address?.toLowerCase();
    const amount = ((tx.amount ?? 0) / MICROUNITS).toFixed(6);
    const peer = (out ? tx.recipient : tx.sender) || '—';
    const short = peer.length > 16 ? peer.slice(0, 10) + '…' + peer.slice(-6) : peer;
    const time = tx.timestamp ? new Date(tx.timestamp * 1000).toLocaleString() : '';
    return `
      <div class="tx-item">
        <span class="tx-dir" style="font-size:1.2rem; color:var(--text-secondary)">${out ? '↑' : '↓'}</span>
        <div class="tx-info" style="flex:1; margin-left:12px;">
          <div class="tx-addr" style="font-family:var(--mono); font-size:0.75rem; color:var(--text-secondary)">
            ${out ? 'To:' : 'From:'} 
            ${state.settings.explorer_url ? `<a href="${state.settings.explorer_url}/address/${peer}" target="_blank" style="color:var(--text-secondary);text-decoration:none;">${escapeHtml(short)}</a>` : escapeHtml(short)}
          </div>
          <div class="tx-time" style="font-size:0.75rem; color:var(--text-muted)">
            ${escapeHtml(time)}
            ${state.settings.explorer_url && (tx.signature || tx.hash) ? `<a href="${state.settings.explorer_url}/tx/${tx.signature || tx.hash}" target="_blank" style="color:var(--cyan);text-decoration:none;margin-left:4px;" title="View on explorer">↗</a>` : ''}
          </div>
        </div>
        <span class="tx-amount" style="font-weight:600; color: ${out ? 'var(--text-primary)' : 'var(--success)'}">${out ? '-' : '+'}${escapeHtml(amount)} QUA</span>
      </div>`;
  }).join('');
}

// ── Send ──────────────────────────────────────────────────────────────────────

async function sendTransaction() {
  const toEl = document.getElementById('send-to');
  const amEl = document.getElementById('send-amount');
  const feeEl = document.getElementById('send-fee');
  const pwdEl = document.getElementById('send-password');
  if (!toEl || !amEl || !feeEl || !pwdEl) return;
  const to = toEl.value.trim();
  const amount = parseFloat(amEl.value);
  const fee = parseFloat(feeEl.value);
  const password = pwdEl.value;
  const errEl = document.getElementById('send-error');
  const succEl = document.getElementById('send-success');
  if (errEl) errEl.classList.add('hidden');
  if (succEl) succEl.classList.add('hidden');

  if (!to.startsWith('0x') && !to.startsWith('ms')) {
    if (errEl) { errEl.textContent = 'Invalid address'; errEl.classList.remove('hidden'); }
    return;
  }
  if (isNaN(amount) || amount <= 0) {
    if (errEl) { errEl.textContent = 'Invalid amount'; errEl.classList.remove('hidden'); }
    return;
  }
  try {
    const wallet = await loadWalletData(password);
    if (!wasm) throw new Error('WASM not loaded');
    const timestamp = Math.floor(Date.now() / 1000);
    const tx = {
      sender: state.address, recipient: to,
      amount: Math.round(amount * MICROUNITS),
      fee: Math.round(fee * MICROUNITS),
      nonce: timestamp, timestamp,
      signature: '', public_key: state.publicKey,
      lock_time: 0, tx_type: 'Transfer', sig_scheme: 'Falcon512',
    };
    const payload = `${tx.sender}:${tx.recipient}:${tx.amount}:${tx.timestamp}:${tx.fee}:${tx.nonce}`;
    const hexPayload = Array.from(new TextEncoder().encode(payload))
      .map(b => b.toString(16).padStart(2, '0')).join('');
    tx.signature = wasm.sign_transaction(hexPayload, wallet.skHex);

    const resp = await fetch(rpcUrl('/transactions'), {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx),
    });
    if (!resp.ok) throw new Error(await resp.text());
    if (succEl) { succEl.textContent = 'Transaction sent'; succEl.classList.remove('hidden'); }
    toast('Transaction sent');
    toEl.value = ''; amEl.value = ''; pwdEl.value = '';
    setTimeout(() => { closePanel('send-panel'); refreshBalance(); loadHistory(); }, 2000);
  } catch (e) {
    if (errEl) {
      errEl.textContent = e.name === 'OperationError' ? 'Wrong password' : 'Error: ' + e.message;
      errEl.classList.remove('hidden');
    }
  }
}

// ── QR ────────────────────────────────────────────────────────────────────────

function renderQr() {
  const c = document.getElementById('qr-container');
  if (!c) return;
  const addr = state.address || '';
  c.innerHTML = `<div style="padding:16px;text-align:center;font-size:0.68rem;font-family:monospace;color:#333;word-break:break-all;max-width:180px">${escapeHtml(addr)}</div>`;
}

// ── Security & Export Keys ────────────────────────────────────────────────────

async function revealPrivateKey() {
  const pwdEl = document.getElementById('export-password');
  const errEl = document.getElementById('export-error');
  if (!pwdEl || !errEl) return;
  const password = pwdEl.value;
  errEl.classList.add('hidden');

  if (!password) { errEl.textContent = 'Enter password'; errEl.classList.remove('hidden'); return; }

  try {
    const wallet = await loadWalletData(password);
    document.getElementById('export-pw-group').classList.add('hidden');
    const resultBox = document.getElementById('export-key-result');
    resultBox.classList.remove('hidden');
    document.getElementById('export-key-text').textContent = wallet.skHex;
  } catch (e) {
    errEl.textContent = 'Incorrect Password';
    errEl.classList.remove('hidden');
  }
}

function copyPrivateKey() {
  const keyEl = document.getElementById('export-key-text');
  if (keyEl && keyEl.textContent) {
    navigator.clipboard.writeText(keyEl.textContent).then(() => toast('Private Key Copied!'));
  }
}

// ── Lock / delete ─────────────────────────────────────────────────────────────

function lockWallet() {
  state.secretKey = null; state.publicKey = null;
  state.address = null; state.balance = 0; state.txHistory = [];
  showScreen('screen-welcome'); toast('Locked');
}

function deleteWallet() {
  if (!confirm('Remove account permanently? Make sure you have your mnemonic or private key backed up!')) return;
  storageRemove(STORAGE_KEY);
  lockWallet(); toast('Account removed');
}

function copyAddress() {
  if (!state.address) return;
  navigator.clipboard.writeText(state.address).then(() => toast('Address copied'));
}

// ── Settings ──────────────────────────────────────────────────────────────────

async function loadSettings() {
  const s = await storageGet(SETTINGS_KEY);
  if (s) {
    state.settings.rpc_url = s.rpc_url || 'http://localhost:3000';
    state.settings.explorer_url = s.explorer_url || 'https://explorer.quantachain.org';
    state.settings.network = s.network || 'testnet';
  }
}

async function saveSettings() {
  const urlEl = document.getElementById('rpc-url');
  const expEl = document.getElementById('explorer-url');
  const selEl = document.getElementById('network-select');
  if (urlEl) state.settings.rpc_url = urlEl.value.trim() || 'http://localhost:3000';
  if (expEl) state.settings.explorer_url = expEl.value.trim() || '';
  if (selEl) state.settings.network = selEl.value;
  await storageSet(SETTINGS_KEY, state.settings);
  updateMainUI(); closePanel('settings-panel');
  toast('Settings saved'); refreshBalance();
}

// ── Boot & Event Binders ──────────────────────────────────────────────────────

window.addEventListener('DOMContentLoaded', async () => {
  await loadWasm();

  const b = (id, fn) => { const el = document.getElementById(id); if (el) el.addEventListener('click', fn); };

  const isPopup = window.innerWidth <= 400 && document.body.clientHeight < window.screen.height;

  b('btn-create', () => {
    if (isPopup) chrome.tabs.create({ url: chrome.runtime.getURL('popup.html?flow=create') });
    else showScreen('screen-create-warn');
  });

  b('btn-import', () => {
    if (isPopup) chrome.tabs.create({ url: chrome.runtime.getURL('popup.html?flow=import') });
    else showScreen('screen-import');
  });

  document.querySelectorAll('.back-btn').forEach(btn => {
    btn.addEventListener('click', (e) => showScreen(e.target.dataset.target));
  });

  const chk = document.getElementById('chk-understand');
  if (chk) chk.addEventListener('change', toggleCreateBtn);

  b('btn-generate-wallet', createWallet);
  b('btn-copy-mnemonic', copyMnemonic);
  b('btn-toggle-mnemonic', () => {
    const grid = document.getElementById('mnemonic-grid');
    if (grid) grid.classList.toggle('blur-words');
  });
  b('btn-written-down', () => { showScreen('screen-confirm'); setupConfirmInputs(); });
  b('btn-verify-continue', confirmMnemonic);

  const pw1El = document.getElementById('pw-new');
  if (pw1El) pw1El.addEventListener('input', checkPasswordStrength);
  b('btn-toggle-pw1', () => togglePw('pw-new'));
  b('btn-toggle-pw2', () => togglePw('pw-confirm'));
  b('btn-set-password', setPassword);

  const impPhr = document.getElementById('import-phrase');
  if (impPhr) impPhr.addEventListener('input', validateImportPhrase);
  b('btn-import-go', importWallet);

  // New UI binds
  b('btn-show-account', () => showPanel('account-panel'));
  b('header-btn-settings', () => showPanel('settings-panel'));
  b('btn-network-toggle', () => showPanel('settings-panel'));

  b('btn-main-copy-addr', copyAddress);
  b('btn-menu-copy', copyAddress);

  b('btn-action-send', () => showPanel('send-panel'));
  b('btn-action-receive', () => showPanel('receive-panel'));
  b('btn-action-refresh', refreshBalance);

  b('btn-show-export', () => showPanel('export-panel'));
  b('btn-reveal-key', revealPrivateKey);
  b('btn-copy-export', copyPrivateKey);

  b('btn-lock-wallet', lockWallet);
  b('btn-delete-wallet', deleteWallet);
  b('btn-save-settings', saveSettings);
  b('btn-exec-send', sendTransaction);
  b('btn-receive-copy', copyAddress);

  document.querySelectorAll('.tab').forEach(btn => {
    btn.addEventListener('click', (e) => switchTab(e.target.dataset.target, e.target));
  });

  document.querySelectorAll('.close-panel').forEach(btn => {
    btn.addEventListener('click', (e) => closePanel(e.target.dataset.panel));
  });

  const overlay = document.getElementById('overlay');
  if (overlay) overlay.addEventListener('click', closeAllPanels);

  const paramFlow = new URLSearchParams(window.location.search).get('flow');

  chrome.runtime.sendMessage({ type: 'GET_LOCK_STATE' }, async (resp) => {
    if (resp?.locked && !paramFlow) { showUnlockScreen(''); return; }

    if (paramFlow === 'create') {
      showScreen('screen-create-warn');
    } else if (paramFlow === 'import') {
      showScreen('screen-import');
    } else if (await walletExists()) {
      const { address } = await getPublicInfo();
      showUnlockScreen(address || '');
    } else {
      showScreen('screen-welcome');
    }
  });
});

// ── Unlock ────────────────────────────────────────────────────────────────────

function showUnlockScreen(address) {
  let s = document.getElementById('screen-unlock');
  if (!s) {
    s = document.createElement('div');
    s.id = 'screen-unlock'; s.className = 'screen';
    s.innerHTML = `
      <div class="card-page" style="text-align:center">
        <div class="logo-wrap" style="margin:0 auto 16px;width:52px;height:52px;display:flex;align-items:center;justify-content:center;">
          <img src="icons/quanta-transparent-bg-logo.png" style="width:50px;height:50px;object-fit:contain; filter: drop-shadow(0 0 10px rgba(255,255,255,0.2));">
        </div>
        <h2>Quanta Wallet</h2>
        <p class="subtitle">Welcome back</p>
        <p style="font-family:var(--mono);font-size:0.75rem;color:var(--text-muted);margin-bottom:24px;word-break:break-all">
          ${address ? escapeHtml(address.slice(0, 12) + '…' + address.slice(-6)) : ''}
        </p>
        <div class="form-group" style="text-align:left">
          <label>Password</label>
          <div class="input-wrap">
            <input id="unlock-pw" type="password" placeholder="Your wallet password">
            <button id="btn-toggle-unlock" class="eye-btn">Show</button>
          </div>
        </div>
        <div id="unlock-error" class="error-msg hidden">Wrong password</div>
        <button id="btn-unlock-exec" class="btn btn-primary full-width" style="margin-top:10px;">Unlock</button>
        <hr class="divider" style="margin:20px 0">
        <button id="btn-unlock-diff" class="btn btn-ghost btn-sm" style="width:100%;">Use Different Wallet</button>
      </div>`;
    document.body.appendChild(s);

    const pw = document.getElementById('unlock-pw');
    if (pw) pw.addEventListener('keydown', (e) => { if (e.key === 'Enter') unlockWallet(); });
    const bToggle = document.getElementById('btn-toggle-unlock');
    if (bToggle) bToggle.addEventListener('click', () => togglePw('unlock-pw'));
    const bExec = document.getElementById('btn-unlock-exec');
    if (bExec) bExec.addEventListener('click', unlockWallet);
    const bDiff = document.getElementById('btn-unlock-diff');
    if (bDiff) bDiff.addEventListener('click', () => {
      if (confirm('Are you sure? Unlocking a different wallet will discard your current keys on this device setup. You MUST have the private key or mnemonic saved.')) {
        showScreen('screen-welcome');
      }
    });
  }
  showScreen('screen-unlock');
}

async function unlockWallet() {
  const pwEl = document.getElementById('unlock-pw');
  if (!pwEl) return;
  const pw = pwEl.value;
  const errEl = document.getElementById('unlock-error');
  if (errEl) errEl.classList.add('hidden');
  try {
    await loadWalletData(pw);
    pingActivity();
    await enterMain();
  } catch {
    if (errEl) errEl.classList.remove('hidden');
  }
}
