/**
 * Quanta Wallet Extension — popup.js
 *
 * Extension-aware version of wallet.js.
 * Key differences from web wallet:
 *  - Uses chrome.storage.local instead of localStorage
 *  - Pings background service worker to reset auto-lock timer
 *  - Checks lock state on popup open
 *  - WASM loaded from extension's pkg/ folder
 */

'use strict';

const STORAGE_KEY  = 'quanta_wallet_v1';
const SETTINGS_KEY = 'quanta_settings_v1';
const MICROUNITS   = 1_000_000;

let state = {
  publicKey: null,
  secretKey: null,
  address:   null,
  balance:   0,
  txHistory: [],
  mnemonic:  null,
  settings: {
    rpc_url: 'http://localhost:3000',
    network: 'mainnet',
  },
};

let wasm = null;

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
    console.log('[Quanta] WASM loaded — Falcon-512 active');
  } catch (e) {
    console.warn('[Quanta] WASM not loaded:', e.message);
    wasm = null;
  }
}

// ── Activity ping (resets auto-lock) ─────────────────────────────────────────

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
  closeAllPanels();
  document.getElementById(id).classList.add('open');
  document.getElementById('overlay').classList.remove('hidden');
  if (id === 'receive-panel') renderQr();
}

function closePanel(id) {
  document.getElementById(id).classList.remove('open');
  if (!document.querySelector('.side-panel.open'))
    document.getElementById('overlay').classList.add('hidden');
}

function closeAllPanels() {
  document.querySelectorAll('.side-panel').forEach(p => p.classList.remove('open'));
  document.getElementById('overlay').classList.add('hidden');
}

// ── Toast ─────────────────────────────────────────────────────────────────────

let toastTimer = null;
function toast(msg, ms = 3000) {
  const el = document.getElementById('toast');
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
  document.getElementById('btn-show-mnemonic').disabled =
    !document.getElementById('chk-understand').checked;
}

async function createWallet() {
  showScreen('screen-loading');
  document.getElementById('loading-msg').textContent = 'Generating Falcon-512 keys…';
  try {
    let mnemonicPhrase, pkHex, skHex, address;
    if (wasm) {
      const result = wasm.generate_wallet();
      mnemonicPhrase = result.mnemonic;
      pkHex    = result.public_key;
      skHex    = result.secret_key;
      address  = result.address;
    } else {
      throw new Error('WASM not loaded — cannot generate PQC keys');
    }
    state.mnemonic  = mnemonicPhrase;
    state.publicKey = pkHex;
    state.secretKey = skHex;
    state.address   = address;
    renderMnemonicGrid(mnemonicPhrase);
    showScreen('screen-mnemonic');
  } catch (e) {
    toast('❌ ' + e.message);
    showScreen('screen-create-warn');
  }
}

function renderMnemonicGrid(phrase) {
  const words = phrase.split(' ');
  document.getElementById('mnemonic-grid').innerHTML = words.map((w, i) => `
    <div class="mnemonic-word">
      <span class="word-num">${i + 1}.</span>
      <span class="word-text">${w}</span>
    </div>`).join('');
}

function copyMnemonic() {
  if (state.mnemonic)
    navigator.clipboard.writeText(state.mnemonic).then(() => toast('✅ Copied'));
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
  document.getElementById('confirm-inputs').innerHTML = confirmPositions.map(i => `
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
  if (!ok) { errEl.classList.remove('hidden'); return; }
  errEl.classList.add('hidden');
  showScreen('screen-password');
}

// ── Password ──────────────────────────────────────────────────────────────────

function checkPasswordStrength() {
  const pw = document.getElementById('pw-new').value;
  const fill = document.getElementById('strength-fill');
  const label = document.getElementById('strength-label');
  let score = 0;
  if (pw.length >= 8)  score++;
  if (pw.length >= 12) score++;
  if (/[A-Z]/.test(pw)) score++;
  if (/[0-9]/.test(pw)) score++;
  if (/[^A-Za-z0-9]/.test(pw)) score++;
  const widths  = ['0%','20%','40%','65%','85%','100%'];
  const colors  = ['#ff4d6a','#ff4d6a','#ffb830','#ffb830','#00ff88','#00d4ff'];
  const labels  = ['','Weak','Fair','Good','Strong','Very Strong'];
  fill.style.width = widths[score]; fill.style.background = colors[score];
  label.textContent = labels[score]; label.style.color = colors[score];
}

function togglePw(id) {
  const el = document.getElementById(id);
  el.type = el.type === 'password' ? 'text' : 'password';
}

async function setPassword() {
  const pw1 = document.getElementById('pw-new').value;
  const pw2 = document.getElementById('pw-confirm').value;
  const errEl = document.getElementById('pw-error');
  if (pw1 !== pw2) { errEl.classList.remove('hidden'); return; }
  if (pw1.length < 8) { toast('Min 8 characters'); return; }
  errEl.classList.add('hidden');
  showScreen('screen-loading');
  document.getElementById('loading-msg').textContent = 'Encrypting wallet…';
  try {
    await saveWallet(state.secretKey, state.publicKey, state.address, state.mnemonic, pw1);
    state.secretKey = null; state.mnemonic = null;
    await enterMain();
  } catch (e) {
    toast('❌ ' + e.message); showScreen('screen-password');
  }
}

// ── Import ────────────────────────────────────────────────────────────────────

function validateImportPhrase() {
  const phrase = document.getElementById('import-phrase').value.trim();
  const words  = phrase.split(/\s+/).filter(Boolean);
  const valid  = words.length === 24 && (wasm ? wasm.validate_mnemonic(phrase) : true);
  document.getElementById('import-valid').classList.toggle('hidden', !valid);
  document.getElementById('import-invalid').classList.toggle('hidden', valid || phrase === '');
  document.getElementById('btn-import-go').disabled = !valid;
}

async function importWallet() {
  const phrase     = document.getElementById('import-phrase').value.trim();
  const passphrase = document.getElementById('import-passphrase').value;
  const password   = document.getElementById('import-password').value;
  const errEl      = document.getElementById('import-error');
  errEl.classList.add('hidden');
  if (password.length < 8) {
    errEl.textContent = 'Password must be at least 8 characters';
    errEl.classList.remove('hidden'); return;
  }
  showScreen('screen-loading');
  document.getElementById('loading-msg').textContent = 'Restoring wallet…';
  try {
    if (!wasm) throw new Error('WASM not loaded');
    const result = wasm.import_wallet(phrase, passphrase, 0);
    await saveWallet(result.secret_key, result.public_key, result.address, phrase, password);
    await enterMain();
  } catch (e) {
    errEl.textContent = '❌ ' + e.message;
    errEl.classList.remove('hidden');
    showScreen('screen-import');
  }
}

// ── Encrypted storage (Web Crypto AES-GCM + PBKDF2) ─────────────────────────

async function deriveKey(password, salt) {
  const enc  = new TextEncoder();
  const base = await crypto.subtle.importKey('raw', enc.encode(password), 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 250_000, hash: 'SHA-256' },
    base,
    { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']
  );
}

async function saveWallet(skHex, pkHex, address, mnemonic, password) {
  const salt  = crypto.getRandomValues(new Uint8Array(16));
  const iv    = crypto.getRandomValues(new Uint8Array(12));
  const key   = await deriveKey(password, salt);
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
  const iv   = new Uint8Array(stored.iv);
  const data = new Uint8Array(stored.data);
  const key  = await deriveKey(password, salt);
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
  state.address   = address;
  state.publicKey = pkHex;
  await loadSettings();
  updateMainUI();
  showScreen('screen-main');
  await refreshBalance();
  await loadHistory();
}

function updateMainUI() {
  const a = state.address || '';
  const addrEl = document.getElementById('wallet-address');
  if (addrEl) addrEl.textContent = a;
  const rAddr = document.getElementById('receive-address-text');
  if (rAddr) rAddr.textContent = a;
  document.getElementById('asset-bal-val').textContent = (state.balance / MICROUNITS).toFixed(6);
  document.getElementById('network-badge').textContent =
    state.settings.network === 'testnet' ? 'Testnet' : 'Mainnet';
  document.getElementById('rpc-url').value = state.settings.rpc_url;
  document.getElementById('network-select').value = state.settings.network;
}

// ── Node API ──────────────────────────────────────────────────────────────────

function rpcUrl(path) {
  return (state.settings.rpc_url || 'http://localhost:3000').replace(/\/$/, '') + path;
}

async function refreshBalance() {
  if (!state.address) return;
  try {
    const r    = await fetch(rpcUrl(`/balance/${state.address}`));
    const data = await r.json();
    state.balance = data.balance ?? data.amount ?? 0;
    document.getElementById('balance-val').textContent =
      (state.balance / MICROUNITS).toFixed(6);
    document.getElementById('asset-bal-val').textContent =
      (state.balance / MICROUNITS).toFixed(6);
  } catch {
    document.getElementById('balance-val').textContent = 'Node offline';
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
  } catch {}
}

function renderHistory() {
  const list = document.getElementById('tx-list');
  if (!state.txHistory.length) {
    list.innerHTML = '<div class="tx-empty">No transactions yet</div>'; return;
  }
  list.innerHTML = state.txHistory.slice(0, 30).map(tx => {
    const out    = tx.sender?.toLowerCase() === state.address?.toLowerCase();
    const amount = ((tx.amount ?? 0) / MICROUNITS).toFixed(6);
    const peer   = (out ? tx.recipient : tx.sender) || '—';
    const short  = peer.length > 16 ? peer.slice(0,10) + '…' + peer.slice(-6) : peer;
    const time   = tx.timestamp ? new Date(tx.timestamp * 1000).toLocaleString() : '';
    return `
      <div class="tx-item">
        <span class="tx-dir">${out ? '↑' : '↓'}</span>
        <div class="tx-info">
          <div class="tx-addr">${out ? 'To:' : 'From:'} ${short}</div>
          <div class="tx-time">${time}</div>
        </div>
        <span class="tx-amount ${out ? 'outgoing' : 'incoming'}">${out ? '-' : '+'}${amount} QUA</span>
      </div>`;
  }).join('');
}

// ── Send ──────────────────────────────────────────────────────────────────────

async function sendTransaction() {
  const to       = document.getElementById('send-to').value.trim();
  const amount   = parseFloat(document.getElementById('send-amount').value);
  const fee      = parseFloat(document.getElementById('send-fee').value);
  const password = document.getElementById('send-password').value;
  const errEl    = document.getElementById('send-error');
  const succEl   = document.getElementById('send-success');
  errEl.classList.add('hidden'); succEl.classList.add('hidden');

  if (!to.startsWith('0x') && !to.startsWith('ms')) {
    errEl.textContent = 'Invalid address'; errEl.classList.remove('hidden'); return;
  }
  if (isNaN(amount) || amount <= 0) {
    errEl.textContent = 'Invalid amount'; errEl.classList.remove('hidden'); return;
  }
  try {
    const wallet   = await loadWalletData(password);
    if (!wasm) throw new Error('WASM not loaded');
    const timestamp = Math.floor(Date.now() / 1000);
    const tx = {
      sender: state.address, recipient: to,
      amount: Math.round(amount * MICROUNITS),
      fee:    Math.round(fee * MICROUNITS),
      nonce: timestamp, timestamp,
      signature: '', public_key: state.publicKey,
      lock_time: 0, tx_type: 'Transfer', sig_scheme: 'Falcon512',
    };
    const payload = `${tx.sender}:${tx.recipient}:${tx.amount}:${tx.timestamp}:${tx.fee}:${tx.nonce}`;
    const hexPayload = Array.from(new TextEncoder().encode(payload))
      .map(b => b.toString(16).padStart(2,'0')).join('');
    tx.signature = wasm.sign_transaction(hexPayload, wallet.skHex);

    const resp = await fetch(rpcUrl('/transactions'), {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx),
    });
    if (!resp.ok) throw new Error(await resp.text());
    succEl.textContent = '✅ Sent!'; succEl.classList.remove('hidden');
    toast('✅ Transaction sent!');
    ['send-to','send-amount','send-password'].forEach(id => document.getElementById(id).value = '');
    setTimeout(() => { closePanel('send-panel'); refreshBalance(); loadHistory(); }, 2000);
  } catch (e) {
    errEl.textContent = e.name === 'OperationError' ? '❌ Wrong password' : '❌ ' + e.message;
    errEl.classList.remove('hidden');
  }
}

// ── QR ────────────────────────────────────────────────────────────────────────

function renderQr() {
  const c = document.getElementById('qr-container');
  const addr = state.address || '';
  c.innerHTML = `<div style="padding:16px;text-align:center;font-size:0.68rem;font-family:monospace;color:#333;word-break:break-all;max-width:180px">${addr}</div>`;
}

// ── Lock / delete ─────────────────────────────────────────────────────────────

function lockWallet() {
  state.secretKey = null; state.publicKey = null;
  state.address = null; state.balance = 0; state.txHistory = [];
  showScreen('screen-welcome'); toast('🔒 Locked');
}

function deleteWallet() {
  if (!confirm('Delete ALL wallet data? Make sure you have your mnemonic backed up!')) return;
  storageRemove(STORAGE_KEY);
  lockWallet(); toast('🗑 Wallet deleted');
}

function exportWallet() { toast('📤 Use the web wallet for full export'); }

function copyAddress() {
  if (!state.address) return;
  navigator.clipboard.writeText(state.address).then(() => toast('📋 Address copied'));
}

// ── Settings ──────────────────────────────────────────────────────────────────

async function loadSettings() {
  const s = await storageGet(SETTINGS_KEY);
  if (s) { state.settings.rpc_url = s.rpc_url || 'http://localhost:3000'; state.settings.network = s.network || 'mainnet'; }
}

async function saveSettings() {
  state.settings.rpc_url = document.getElementById('rpc-url').value.trim() || 'http://localhost:3000';
  state.settings.network = document.getElementById('network-select').value;
  await storageSet(SETTINGS_KEY, state.settings);
  updateMainUI(); closePanel('settings-panel');
  toast('✅ Settings saved'); refreshBalance();
}

function setNetwork() { state.settings.network = document.getElementById('network-select').value; }

// ── Boot ──────────────────────────────────────────────────────────────────────

window.addEventListener('DOMContentLoaded', async () => {
  await loadWasm();

  // Wire confirm-mnemonic step
  document.querySelectorAll('[onclick*="screen-confirm"]').forEach(btn => {
    btn.addEventListener('click', () => setTimeout(setupConfirmInputs, 50));
  });

  // Check auto-lock state
  chrome.runtime.sendMessage({ type: 'GET_LOCK_STATE' }, async (resp) => {
    if (resp?.locked) { showUnlockScreen(''); return; }

    if (await walletExists()) {
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
        <div class="mini-logo" style="margin:0 auto 16px;width:52px;height:52px;border-radius:14px;font-size:1.3rem;display:flex;align-items:center;justify-content:center;background:linear-gradient(135deg,#00d4ff,#00ff88);color:#000;font-weight:800">Q</div>
        <h2>Quanta Wallet</h2>
        <p class="subtitle">Welcome back</p>
        <p style="font-family:var(--mono);font-size:0.7rem;color:var(--text-muted);margin-bottom:24px;word-break:break-all">
          ${address ? address.slice(0,12)+'…'+address.slice(-6) : ''}
        </p>
        <div class="form-group" style="text-align:left">
          <label>Password</label>
          <div class="input-wrap">
            <input id="unlock-pw" type="password" placeholder="Your wallet password" onkeydown="if(event.key==='Enter')unlockWallet()">
            <button class="eye-btn" onclick="togglePw('unlock-pw')">👁</button>
          </div>
        </div>
        <div id="unlock-error" class="error-msg hidden">Wrong password</div>
        <button class="btn btn-primary" onclick="unlockWallet()">🔓 Unlock</button>
        <hr class="divider" style="margin:20px 0">
        <button class="btn btn-ghost btn-sm" style="width:auto" onclick="showScreen('screen-welcome')">Use Different Wallet</button>
      </div>`;
    document.body.appendChild(s);
  }
  showScreen('screen-unlock');
}

async function unlockWallet() {
  const pw = document.getElementById('unlock-pw').value;
  document.getElementById('unlock-error').classList.add('hidden');
  try {
    await loadWalletData(pw);
    pingActivity();
    await enterMain();
  } catch {
    document.getElementById('unlock-error').classList.remove('hidden');
  }
}
