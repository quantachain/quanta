import init, { WalletKeys } from './pkg/quanta_wallet_wasm.js';

// --- STATE ---
let wallet = null;
let tempPassword = null; // Store temporarily during creation
let encryptedVault = null;

// --- ICONS ---
const ICONS = {
    settings: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>`,
    back: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>`,
    scan: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7V5a2 2 0 0 1 2-2h2"></path><path d="M17 3h2a2 2 0 0 1 2 2v2"></path><path d="M21 17v2a2 2 0 0 1-2 2h-2"></path><path d="M7 21H5a2 2 0 0 1-2-2v-2"></path></svg>`
};

// --- DOM ---
const $ = (id) => document.getElementById(id);

// --- ROUTER & NAVIGATION ---
const router = {
    history: [],

    // Navigate to a new view (push to stack)
    push(viewId, title = "Wallet") {
        this.history.push({ id: viewId, title: title });
        this.render();
    },

    // Replace current view (no history) - used for auth/home
    replace(viewId, title = "Wallet") {
        this.history = [{ id: viewId, title: title }];
        this.render();
    },

    // Go back
    pop() {
        if (this.history.length > 1) {
            this.history.pop();
            this.render();
        }
    },

    render() {
        const current = this.history[this.history.length - 1];
        const viewId = current.id;
        const title = current.title;

        // 1. Switch View
        document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
        $(viewId).classList.add('active');
        $('pageTitle').innerText = title;

        // 2. Update Header
        const leftBtn = $('headerLeft');

        // Hide header actions on onboarding/login
        if (['onboardingView', 'createPasswordView', 'loginView'].includes(viewId)) {
            leftBtn.innerHTML = '';
            leftBtn.onclick = null;
            $('headerRight').innerHTML = ''; // Clear any right actions
            return;
        }

        // Wallet Home Logic (Root)
        if (this.history.length === 1 && viewId === 'walletView') {
            leftBtn.innerHTML = ICONS.settings;
            leftBtn.onclick = () => router.push('settingsView', 'Settings');
            $('headerRight').innerHTML = ICONS.scan; // Example: Scan/Connect icon
        } else {
            // Sub-pages: Show Back Button
            leftBtn.innerHTML = ICONS.back;
            leftBtn.onclick = () => router.pop();
            $('headerRight').innerHTML = '';
        }
    }
};

// --- CRYPTO HELPERS (AES-GCM) ---
// We derive a key from the password using PBKDF2, then encrypt/decrypt.
async function getCryptoKey(password, salt) {
    const enc = new TextEncoder();
    const keyMaterial = await crypto.subtle.importKey("raw", enc.encode(password), "PBKDF2", false, ["deriveKey"]);
    return crypto.subtle.deriveKey(
        { name: "PBKDF2", salt: salt, iterations: 100000, hash: "SHA-256" },
        keyMaterial,
        { name: "AES-GCM", length: 256 },
        true,
        ["encrypt", "decrypt"]
    );
}

async function encryptVault(dataObj, password) {
    const salt = crypto.getRandomValues(new Uint8Array(16));
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const key = await getCryptoKey(password, salt);

    const enc = new TextEncoder();
    const encoded = enc.encode(JSON.stringify(dataObj));
    const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: iv }, key, encoded);

    // Store as JSON: salt, iv, ciphertext (all hex)
    return {
        salt: toHex(salt),
        iv: toHex(iv),
        data: toHex(new Uint8Array(ciphertext))
    };
}

async function decryptVault(vault, password) {
    const salt = fromHex(vault.salt);
    const iv = fromHex(vault.iv);
    const data = fromHex(vault.data);
    const key = await getCryptoKey(password, salt);

    try {
        const decrypted = await crypto.subtle.decrypt({ name: "AES-GCM", iv: iv }, key, data);
        const dec = new TextDecoder();
        return JSON.parse(dec.decode(decrypted));
    } catch (e) {
        throw new Error("Incorrect password");
    }
}

function toHex(buffer) {
    return Array.from(buffer).map(b => b.toString(16).padStart(2, '0')).join('');
}
function fromHex(hexString) {
    return new Uint8Array(hexString.match(/.{1,2}/g).map(byte => parseInt(byte, 16)));
}

// --- LOGIC ---
document.addEventListener('DOMContentLoaded', async () => {
    await init();

    // Check if wallet exists
    const storedVault = localStorage.getItem('qua_vault');
    if (storedVault) {
        encryptedVault = JSON.parse(storedVault);
        router.replace('loginView', 'Unlock');
    } else {
        router.replace('onboardingView', 'Welcome');
    }
});

// --- ONBOARDING FLOW ---
$('startSetupBtn').onclick = () => router.push('createPasswordView', 'Setup');
$('importWalletBtn').onclick = () => alert("Import feature coming soon!");

// Password Creation
const checkPasswordForm = () => {
    const p1 = $('newPasswordInput').value;
    const p2 = $('confirmPasswordInput').value;
    const term = $('termsCheck').checked;
    const btn = $('savePasswordBtn');

    if (p1.length >= 8 && p1 === p2 && term) {
        btn.disabled = false;
        btn.style.opacity = 1;
    } else {
        btn.disabled = true;
        btn.style.opacity = 0.5;
    }
};
$('newPasswordInput').oninput = checkPasswordForm;
$('confirmPasswordInput').oninput = checkPasswordForm;
$('termsCheck').onchange = checkPasswordForm;

$('savePasswordBtn').onclick = async () => {
    const password = $('newPasswordInput').value;
    const btn = $('savePasswordBtn');
    btn.innerText = "Generating Keys...";

    // 1. Generate new keys
    setTimeout(async () => {
        wallet = new WalletKeys();
        const keys = { pk: wallet.get_public_key_hex(), sk: wallet.get_private_key_hex() };

        // 2. Encrypt keys with password
        const vault = await encryptVault(keys, password);
        localStorage.setItem('qua_vault', JSON.stringify(vault));

        // 3. Enter wallet
        updateData();
        router.replace('walletView'); // Replace history so back doesn't go to setup
    }, 100);
};

// Login
$('unlockBtn').onclick = async () => {
    const password = $('loginPasswordInput').value;
    const err = $('loginError');
    const btn = $('unlockBtn');

    if (!encryptedVault) return;

    btn.innerText = "Unlocking...";

    try {
        const keys = await decryptVault(encryptedVault, password);
        wallet = WalletKeys.from_keypair(keys.pk, keys.sk);
        updateData();
        router.replace('walletView');
    } catch (e) {
        btn.innerText = "Unlock";
        err.style.opacity = 1;
        // Shake animation
        $('loginPasswordInput').parentElement.style.borderColor = '#EF4444';
        setTimeout(() => $('loginPasswordInput').parentElement.style.borderColor = 'var(--border)', 2000);
    }
};

$('loginPasswordInput').onkeydown = (e) => {
    if (e.key === 'Enter') $('unlockBtn').click();
};

$('resetWalletLink').onclick = () => {
    if (confirm("Are you sure? This will maintain the current wallet encrypted but start a new setup. If you lost your password, your old wallet is lost.")) {
        localStorage.removeItem('qua_vault');
        location.reload();
    }
};


// --- WALLET VIEWS ---
// Tab Logic
const tabs = document.querySelectorAll('.tab');
tabs[0].onclick = () => {
    tabs[0].classList.add('active'); tabs[1].classList.remove('active');
    $('tokenList').style.display = 'block'; $('activityList').style.display = 'none';
};
tabs[1].onclick = () => {
    tabs[1].classList.add('active'); tabs[0].classList.remove('active');
    $('tokenList').style.display = 'none'; $('activityList').style.display = 'block';
};

$('navSend').onclick = () => router.push('sendView', 'Send QUA');
$('navReceive').onclick = () => router.push('receiveView', 'Receive QUA');

// Send Flow
$('broadcastBtn').onclick = async () => {
    const btn = $('broadcastBtn');
    btn.innerText = "Signing...";
    await new Promise(r => setTimeout(r, 800));
    // Here we would actually use wallet.sign_transaction_hash()
    btn.innerText = "Sent!";
    btn.style.background = "#14F195";
    setTimeout(() => {
        btn.innerText = "Review";
        btn.style.background = "#00E599";
        router.pop(); // Go back to wallet
    }, 1200);
};

// Copy
$('copyBtn').onclick = () => {
    navigator.clipboard.writeText(wallet.get_address());
    $('copyBtn').innerText = "Copied!";
    setTimeout(() => $('copyBtn').innerText = "Copy Address", 1000);
};

// Settings
$('navReveal').onclick = () => {
    $('secretBox').style.filter = "blur(8px)";
    $('secretBox').innerText = "CLICK TO REVEAL";
    router.push('revealView', 'Secret Key');
};
$('navNetwork').onclick = () => router.push('networkView', 'Network');
$('doLogout').onclick = () => {
    wallet = null;
    location.reload();
};

// Reveal
$('secretBox').onclick = () => {
    $('secretBox').style.filter = "none";
    $('secretBox').innerText = wallet.get_private_key_hex();
};

// Network
$('selTestnet').onclick = () => { $('checkTestnet').style.opacity = 1; $('checkLocal').style.opacity = 0; $('currentNetLabel').innerText = "Testnet >"; };
$('selLocal').onclick = () => { $('checkTestnet').style.opacity = 0; $('checkLocal').style.opacity = 1; $('currentNetLabel').innerText = "Localhost >"; };


function updateData() {
    if (!wallet) return;
    const addr = wallet.get_address();
    $('addressDisplay').innerText = addr.substring(0, 6) + "..." + addr.substring(addr.length - 4);
    $('fullAddress').innerText = addr;
    // Mock Balance for UI
    $('balanceDisplay').innerText = "1,240.50";
}


