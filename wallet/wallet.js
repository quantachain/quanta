/**
 * Quanta Wallet — wallet.js
 * Handles: screen navigation, wallet creation/import, local encrypted storage,
 *          transaction signing (via WASM), and node API calls.
 *
 * Storage key layout (localStorage):
 *   quanta_wallet_v1      — JSON: { encrypted_sk, pk_hex, address, accounts[] }
 *   quanta_settings_v1    — JSON: { rpc_url, network }
 */

'use strict';

// ============================================================================
// CONFIG & STATE
// ============================================================================

function escapeHtml(unsafe) {
  if (!unsafe) return '';
  return String(unsafe).replace(/[&<"'>]/g, function (m) {
    return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#039;' }[m];
  });
}

const STORAGE_KEY = 'quanta_wallet_v1';
const SETTINGS_KEY = 'quanta_settings_v1';
const MICROUNITS = 1_000_000; // 1 QUA = 1_000_000 microunits

let state = {
  mnemonic: null,   // transient — only during creation flow
  publicKey: null,   // hex
  secretKey: null,   // hex — transient, zeroize ASAP after use
  address: null,
  balance: 0,
  txHistory: [],
  settings: {
    rpc_url: 'http://localhost:3000',
    explorer_url: 'https://explorer.quantachain.org',
    network: 'testnet',
  },
};

// WASM module reference (loaded async)
let wasm = null;

// ============================================================================
// WASM BOOTSTRAP
// ============================================================================

async function loadWasm() {
  try {
    // In production: built by wasm-pack, loaded from ./pkg/quanta_wasm.js
    const module = await import('./pkg/quanta_wasm.js');
    await module.default();
    wasm = module;
    console.log('[Quanta] WASM loaded — Falcon-512 PQC active');
  } catch (e) {
    console.warn('[Quanta] WASM not loaded — using fallback mode:', e.message);
    wasm = null; // handled gracefully in each function
  }
}

// ============================================================================
// SCREEN NAVIGATION
// ============================================================================

function showScreen(id) {
  document.querySelectorAll('.screen').forEach(s => s.classList.remove('active'));
  const el = document.getElementById(id);
  if (el) {
    el.classList.add('active');
    el.scrollTop = 0;
  }
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
  const anyOpen = document.querySelector('.side-panel.open');
  if (!anyOpen) document.getElementById('overlay').classList.add('hidden');
}

function closeAllPanels() {
  document.querySelectorAll('.side-panel').forEach(p => p.classList.remove('open'));
  document.getElementById('overlay').classList.add('hidden');
}

// ============================================================================
// TOAST
// ============================================================================

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

// ============================================================================
// WALLET CREATION FLOW
// ============================================================================

function toggleCreateBtn() {
  const checked = document.getElementById('chk-understand').checked;
  document.getElementById('btn-show-mnemonic').disabled = !checked;
}

async function createWallet() {
  showScreen('screen-loading');
  document.getElementById('loading-msg').textContent = 'Generating Falcon-512 keys…';

  try {
    let mnemonicPhrase, pkHex, skHex, address;

    if (wasm) {
      const result = wasm.generate_wallet();
      mnemonicPhrase = result.mnemonic;
      pkHex = result.public_key;
      skHex = result.secret_key;
      address = result.address;
    } else {
      // Fallback: generate mnemonic via Web Crypto + display placeholder
      mnemonicPhrase = await generateMnemonicFallback();
      pkHex = null;
      skHex = null;
      address = '0x0000000000000000000000000000000000000000';
      toast('⚠ WASM not loaded — crypto operations limited');
    }

    state.mnemonic = mnemonicPhrase;
    state.publicKey = pkHex;
    state.secretKey = skHex; // transient until password is set
    state.address = address;

    renderMnemonicGrid(mnemonicPhrase);
    showScreen('screen-mnemonic');
  } catch (e) {
    toast('❌ Error: ' + e.message);
    showScreen('screen-create-warn');
  }
}

function renderMnemonicGrid(phrase) {
  const words = phrase.split(' ');
  const grid = document.getElementById('mnemonic-grid');
  grid.innerHTML = words.map((w, i) => `
    <div class="mnemonic-word">
      <span class="word-num">${i + 1}.</span>
      <span class="word-text">${w}</span>
    </div>`).join('');
  grid.classList.remove('blur-words');
}

function copyMnemonic() {
  if (state.mnemonic) {
    navigator.clipboard.writeText(state.mnemonic)
      .then(() => toast('✅ Mnemonic copied'))
      .catch(() => toast('❌ Copy failed'));
  }
}

// --- Confirm step: 3 random word positions ---
const confirmPositions = [];
function showScreen_confirm_init() { } // called via showScreen('screen-confirm') button

// Intercept the "I've Written It Down" button to set up confirm inputs
document.addEventListener('DOMContentLoaded', () => {
  document.querySelector('[onclick="showScreen(\'screen-confirm\')"]')
    ?.addEventListener('click', () => setupConfirmInputs(), { once: false });
});

function setupConfirmInputs() {
  if (!state.mnemonic) return;
  const words = state.mnemonic.split(' ');
  confirmPositions.length = 0;
  // Pick 3 random positions
  while (confirmPositions.length < 3) {
    const r = Math.floor(Math.random() * 24);
    if (!confirmPositions.includes(r)) confirmPositions.push(r);
  }
  confirmPositions.sort((a, b) => a - b);

  const grid = document.getElementById('confirm-inputs');
  grid.innerHTML = confirmPositions.map(i => `
    <div class="confirm-row">
      <span class="confirm-num">Word #${i + 1}</span>
      <input type="text" id="confirm-word-${i}" placeholder="enter word ${i + 1}" autocomplete="off" spellcheck="false">
    </div>`).join('');
}

function confirmMnemonic() {
  if (!state.mnemonic) return;
  const words = state.mnemonic.split(' ');
  const errEl = document.getElementById('confirm-error');

  let allCorrect = true;
  for (const pos of confirmPositions) {
    const input = document.getElementById(`confirm-word-${pos}`);
    if (!input || input.value.trim().toLowerCase() !== words[pos]) {
      allCorrect = false;
      break;
    }
  }

  if (!allCorrect) {
    errEl.classList.remove('hidden');
    return;
  }
  errEl.classList.add('hidden');
  showScreen('screen-password');
}

// ============================================================================
// PASSWORD & ENCRYPTION
// ============================================================================

function checkPasswordStrength() {
  const pw = document.getElementById('pw-new').value;
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
  fill.style.width = widths[score];
  fill.style.background = colors[score];
  label.textContent = labels[score];
  label.style.color = colors[score];
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
  if (pw1.length < 8) { toast('Password must be at least 8 characters'); return; }
  errEl.classList.add('hidden');

  showScreen('screen-loading');
  document.getElementById('loading-msg').textContent = 'Encrypting wallet…';

  try {
    await saveWallet(state.secretKey, state.publicKey, state.address, state.mnemonic, pw1);
    // Zeroize transient secret
    state.secretKey = null;
    state.mnemonic = null;
    await enterMain();
  } catch (e) {
    toast('❌ Error saving wallet: ' + e.message);
    showScreen('screen-password');
  }
}

// ============================================================================
// IMPORT FLOW
// ============================================================================

function validateImportPhrase() {
  const phrase = document.getElementById('import-phrase').value.trim();
  const words = phrase.split(/\s+/).filter(Boolean);
  const valid = words.length === 24 && (wasm ? wasm.validate_mnemonic(phrase) : true);
  document.getElementById('import-valid').classList.toggle('hidden', !valid);
  document.getElementById('import-invalid').classList.toggle('hidden', valid || phrase === '');
  document.getElementById('btn-import-go').disabled = !valid;
}

async function importWallet() {
  const phrase = document.getElementById('import-phrase').value.trim();
  const passphrase = document.getElementById('import-passphrase').value;
  const password = document.getElementById('import-password').value;
  const errEl = document.getElementById('import-error');

  if (password.length < 8) {
    errEl.textContent = 'Password must be at least 8 characters';
    errEl.classList.remove('hidden');
    return;
  }
  errEl.classList.add('hidden');
  showScreen('screen-loading');
  document.getElementById('loading-msg').textContent = 'Restoring wallet from mnemonic…';

  try {
    let pkHex, skHex, address;
    if (wasm) {
      const result = wasm.import_wallet(phrase, passphrase, 0);
      pkHex = result.public_key;
      skHex = result.secret_key;
      address = result.address;
    } else {
      pkHex = null;
      skHex = null;
      address = '0x0000000000000000000000000000000000000000';
      toast('⚠ WASM not loaded');
    }

    await saveWallet(skHex, pkHex, address, phrase, password);
    await enterMain();
  } catch (e) {
    errEl.textContent = 'Import failed: ' + e.message;
    errEl.classList.remove('hidden');
    showScreen('screen-import');
  }
}

// ============================================================================
// ENCRYPTED STORAGE (Web Crypto AES-GCM + PBKDF2)
// ============================================================================

async function deriveKey(password, salt) {
  const enc = new TextEncoder();
  const base = await crypto.subtle.importKey('raw', enc.encode(password), 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 250_000, hash: 'SHA-256' },
    base,
    { name: 'AES-GCM', length: 256 },
    false, ['encrypt', 'decrypt']
  );
}

async function saveWallet(skHex, pkHex, address, mnemonic, password) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(password, salt);
  const enc = new TextEncoder();

  const plaintext = JSON.stringify({ skHex, pkHex, address, mnemonic });
  const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, enc.encode(plaintext));

  const stored = {
    salt: Array.from(salt),
    iv: Array.from(iv),
    data: Array.from(new Uint8Array(ciphertext)),
    address, pkHex,   // stored plaintext for display without decryption
  };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(stored));
}

async function loadWallet(password) {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) throw new Error('No wallet found');

  const stored = JSON.parse(raw);
  const salt = new Uint8Array(stored.salt);
  const iv = new Uint8Array(stored.iv);
  const data = new Uint8Array(stored.data);
  const key = await deriveKey(password, salt);

  const dec = new TextDecoder();
  const buf = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, data);
  return JSON.parse(dec.decode(buf));
}

function walletExists() {
  return !!localStorage.getItem(STORAGE_KEY);
}

function getStoredPublicInfo() {
  try {
    const s = JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}');
    return { address: s.address || null, pkHex: s.pkHex || null };
  } catch { return { address: null, pkHex: null }; }
}

// ============================================================================
// MAIN WALLET VIEW
// ============================================================================

async function enterMain() {
  const { address, pkHex } = getStoredPublicInfo();
  state.address = address;
  state.publicKey = pkHex;

  loadSettings();
  updateMainUI();
  showScreen('screen-main');
  await refreshBalance();
  await loadHistory();
}

function updateMainUI() {
  const addrEl = document.getElementById('wallet-address');
  if (addrEl && state.address) addrEl.textContent = state.address;

  const rAddr = document.getElementById('receive-address-text');
  if (rAddr && state.address) rAddr.textContent = state.address;

  document.getElementById('asset-bal-val').textContent =
    (state.balance / MICROUNITS).toFixed(6);

  document.getElementById('network-badge').textContent =
    state.settings.network === 'testnet' ? 'Testnet' : 'Mainnet';

  const rpcEl = document.getElementById('rpc-url');
  if (rpcEl) rpcEl.value = state.settings.rpc_url;
  const expEl = document.getElementById('explorer-url');
  if (expEl) expEl.value = state.settings.explorer_url || '';
  const netEl = document.getElementById('network-select');
  if (netEl) netEl.value = state.settings.network;
}

// ============================================================================
// NODE API CALLS
// ============================================================================

function rpcUrl(path) {
  const base = (state.settings.rpc_url || 'http://localhost:3000').replace(/\/$/, '');
  return base + path;
}

async function refreshBalance() {
  if (!state.address) return;
  try {
    const r = await fetch(rpcUrl(`/balance/${state.address}`));
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const data = await r.json();
    // Node returns { balance: <microunits_integer> }
    state.balance = data.balance ?? data.amount ?? 0;
    document.getElementById('balance-val').textContent =
      (state.balance / MICROUNITS).toFixed(6);
    document.getElementById('asset-bal-val').textContent =
      (state.balance / MICROUNITS).toFixed(6);
  } catch (e) {
    console.warn('[Quanta] Balance fetch failed:', e.message);
    document.getElementById('balance-val').textContent = 'Node offline';
  }
}

async function loadHistory() {
  if (!state.address) return;
  try {
    const r = await fetch(rpcUrl(`/transactions/${state.address}`));
    if (!r.ok) return;
    const txs = await r.json();
    state.txHistory = Array.isArray(txs) ? txs : (txs.transactions ?? []);
    renderHistory();
  } catch (e) {
    console.warn('[Quanta] History fetch failed:', e.message);
  }
}

function renderHistory() {
  const list = document.getElementById('tx-list');
  if (!state.txHistory.length) {
    list.innerHTML = '<div class="tx-empty">No transactions yet</div>';
    return;
  }
  list.innerHTML = state.txHistory.slice(0, 30).map(tx => {
    const outgoing = tx.sender?.toLowerCase() === state.address?.toLowerCase();
    const amount = ((tx.amount ?? 0) / MICROUNITS).toFixed(6);
    const peer = outgoing ? tx.recipient : tx.sender;
    const short = peer ? peer.slice(0, 10) + '…' + peer.slice(-6) : '—';
    const time = tx.timestamp ? new Date(tx.timestamp * 1000).toLocaleString() : '';
    return `
      <div class="tx-item">
        <span class="tx-dir">${outgoing ? '↑' : '↓'}</span>
        <div class="tx-info">
          <div class="tx-addr">
            ${outgoing ? 'To:' : 'From:'} 
            ${state.settings.explorer_url ? `<a href="${state.settings.explorer_url}/address/${peer}" target="_blank" style="color:var(--text-secondary);text-decoration:none;">${escapeHtml(short)}</a>` : escapeHtml(short)}
          </div>
          <div class="tx-time">
            ${escapeHtml(time)}
            ${state.settings.explorer_url && (tx.signature || tx.hash) ? `<a href="${state.settings.explorer_url}/tx/${tx.signature || tx.hash}" target="_blank" style="color:var(--cyan);text-decoration:none;margin-left:4px;" title="View on explorer">↗</a>` : ''}
          </div>
        </div>
        <span class="tx-amount ${outgoing ? 'outgoing' : 'incoming'}">
          ${outgoing ? '-' : '+'}${escapeHtml(amount)} QUA
        </span>
      </div>`;
  }).join('');
}

// ============================================================================
// SEND TRANSACTION
// ============================================================================

async function sendTransaction() {
  const to = document.getElementById('send-to').value.trim();
  const amount = parseFloat(document.getElementById('send-amount').value);
  const fee = parseFloat(document.getElementById('send-fee').value);
  const password = document.getElementById('send-password').value;
  const errEl = document.getElementById('send-error');
  const successEl = document.getElementById('send-success');

  errEl.classList.add('hidden');
  successEl.classList.add('hidden');

  if (!to.startsWith('0x') && !to.startsWith('ms')) {
    errEl.textContent = 'Invalid recipient address (must start with 0x or ms)';
    errEl.classList.remove('hidden'); return;
  }
  if (isNaN(amount) || amount <= 0) {
    errEl.textContent = 'Invalid amount'; errEl.classList.remove('hidden'); return;
  }

  try {
    // Decrypt wallet to get secret key
    const walletData = await loadWallet(password);
    const skHex = walletData.skHex;

    if (!skHex || !wasm) {
      throw new Error(wasm ? 'No secret key in stored wallet' : 'WASM not loaded — cannot sign');
    }

    const timestamp = Math.floor(Date.now() / 1000);
    const nonce = timestamp; // simplistic; real impl uses per-address nonce

    // Build tx payload matching chain's Transaction struct (JSON-serialised)
    const tx = {
      sender: state.address,
      recipient: to,
      amount: Math.round(amount * MICROUNITS),
      fee: Math.round(fee * MICROUNITS),
      nonce,
      timestamp,
      signature: '',
      public_key: state.publicKey,
      lock_time: 0,
      tx_type: 'Transfer',
      sig_scheme: 'Falcon512',
    };

    // Signing payload: serialize canonical fields
    const signingPayload = buildSigningPayload(tx);
    const signingHex = toHex(new TextEncoder().encode(signingPayload));

    // Sign with Falcon-512 via WASM
    const signatureHex = wasm.sign_transaction(signingHex, skHex);
    tx.signature = signatureHex;

    // Broadcast
    const resp = await fetch(rpcUrl('/transactions'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx),
    });

    if (!resp.ok) {
      const body = await resp.text();
      throw new Error(`Node rejected transaction: ${body}`);
    }

    successEl.textContent = '✅ Transaction broadcast successfully!';
    successEl.classList.remove('hidden');
    toast('✅ Transaction sent!');

    // Clear form
    document.getElementById('send-to').value = '';
    document.getElementById('send-amount').value = '';
    document.getElementById('send-password').value = '';

    setTimeout(() => { closePanel('send-panel'); refreshBalance(); loadHistory(); }, 2000);
  } catch (e) {
    if (e.name === 'OperationError') {
      errEl.textContent = '❌ Wrong password';
    } else {
      errEl.textContent = '❌ ' + e.message;
    }
    errEl.classList.remove('hidden');
  }
}

/** Build the canonical signing payload string to match the chain's get_signing_bytes() */
function buildSigningPayload(tx) {
  return `${tx.sender}:${tx.recipient}:${tx.amount}:${tx.timestamp}:${tx.fee}:${tx.nonce}`;
}

function toHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

// ============================================================================
// QR CODE (via qr-code-styling or simple SVG fallback)
// ============================================================================

function renderQr() {
  const container = document.getElementById('qr-container');
  const addr = state.address || '';
  if (!addr) return;

  // Use qrcode-generator lib if available, else show text
  if (typeof qrcode !== 'undefined') {
    container.innerHTML = '';
    const qr = qrcode(0, 'M');
    qr.addData(addr);
    qr.make();
    container.innerHTML = qr.createImgTag(4, 4);
  } else {
    // Simple ASCII fallback
    container.innerHTML = `
      <div style="padding:16px;text-align:center;font-size:0.7rem;font-family:monospace;color:#333;word-break:break-all;max-width:200px">
        📱 QR<br><br>${addr}
      </div>`;
  }
}

// ============================================================================
// LOCK / DELETE
// ============================================================================

function lockWallet() {
  state.secretKey = null;
  state.publicKey = null;
  state.address = null;
  state.balance = 0;
  state.txHistory = [];
  showScreen('screen-welcome');
  toast('🔒 Wallet locked');
}

function deleteWallet() {
  if (!confirm('⚠ Delete ALL wallet data? This cannot be undone. Make sure you have your mnemonic backed up.')) return;
  localStorage.removeItem(STORAGE_KEY);
  lockWallet();
  toast('🗑 Wallet deleted');
}

function exportWallet() {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) { toast('No wallet data'); return; }
  const blob = new Blob([raw], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `quanta-wallet-${Date.now()}.json`;
  a.click();
  URL.revokeObjectURL(url);
  toast('📤 Wallet exported');
}

// ============================================================================
// UTILITIES
// ============================================================================

function copyAddress() {
  if (!state.address) { toast('No address loaded'); return; }
  navigator.clipboard.writeText(state.address)
    .then(() => toast('📋 Address copied'))
    .catch(() => toast('❌ Copy failed'));
}

// ============================================================================
// SETTINGS
// ============================================================================

function loadSettings() {
  try {
    const s = JSON.parse(localStorage.getItem(SETTINGS_KEY) || '{}');
    state.settings.rpc_url = s.rpc_url || 'http://localhost:3000';
    state.settings.explorer_url = s.explorer_url || 'https://explorer.quantachain.org';
    state.settings.network = s.network || 'testnet';
  } catch { }
}

function saveSettings() {
  state.settings.rpc_url = document.getElementById('rpc-url')?.value.trim() || 'http://localhost:3000';
  state.settings.explorer_url = document.getElementById('explorer-url')?.value.trim() || '';
  state.settings.network = document.getElementById('network-select')?.value || 'testnet';
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(state.settings));
  updateMainUI();
  closePanel('settings-panel');
  toast('✅ Settings saved');
  refreshBalance();
}

function setNetwork() {
  state.settings.network = document.getElementById('network-select').value;
}

// ============================================================================
// MNEMONIC FALLBACK (when WASM not available)
// ============================================================================

async function generateMnemonicFallback() {
  // BIP39 wordlist — first 12 words (simplified, full list loaded dynamically)
  // For production, embed the full BIP39 wordlist
  const entropy = crypto.getRandomValues(new Uint8Array(32));
  // Return hex as placeholder mnemonic string
  return Array.from(entropy).map(_ => 'word').join(' ');
}

// ============================================================================
// BOOT
// ============================================================================

window.addEventListener('DOMContentLoaded', async () => {
  // Wire up the confirm-mnemonic setup when navigating to that screen
  document.querySelectorAll('[onclick*="screen-confirm"]').forEach(btn => {
    btn.addEventListener('click', () => setTimeout(setupConfirmInputs, 50));
  });

  loadSettings();
  await loadWasm();

  if (walletExists()) {
    // Wallet is stored — show lock screen (re-use welcome with "Unlock" context)
    const { address } = getStoredPublicInfo();
    if (address) {
      // Show unlock prompt
      showUnlockScreen(address);
      return;
    }
  }

  showScreen('screen-welcome');
});

// ============================================================================
// UNLOCK SCREEN (injected at runtime if wallet exists)
// ============================================================================

function showUnlockScreen(address) {
  // Create unlock screen dynamically
  const existing = document.getElementById('screen-unlock');
  if (!existing) {
    const s = document.createElement('div');
    s.id = 'screen-unlock';
    s.className = 'screen';
    s.innerHTML = `
      <div class="card-page" style="text-align:center">
        <div class="mini-logo" style="margin:0 auto 16px;width:52px;height:52px;border-radius:14px;font-size:1.3rem;display:flex;align-items:center;justify-content:center;background:linear-gradient(135deg,#00d4ff,#00ff88);color:#000;font-weight:800">Q</div>
        <h2>Unlock Wallet</h2>
        <p class="subtitle" style="margin-bottom:4px">Welcome back</p>
        <p style="font-family:var(--mono);font-size:0.72rem;color:var(--text-muted);margin-bottom:24px;word-break:break-all">
          ${address?.slice(0, 12)}…${address?.slice(-6)}
        </p>
        <div class="form-group" style="text-align:left">
          <label>Password</label>
          <div class="input-wrap">
            <input id="unlock-pw" type="password" placeholder="Your wallet password" onkeydown="if(event.key==='Enter')unlockWallet()">
            <button class="eye-btn" onclick="togglePw('unlock-pw')">👁</button>
          </div>
        </div>
        <div id="unlock-error" class="error-msg hidden">Wrong password. Try again.</div>
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
  const errEl = document.getElementById('unlock-error');
  errEl.classList.add('hidden');
  try {
    await loadWallet(pw); // just to validate password
    await enterMain();
  } catch {
    errEl.classList.remove('hidden');
  }
}
