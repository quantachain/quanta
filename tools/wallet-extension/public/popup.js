import init, { WalletKeys } from '../pkg_v2/quanta_wallet_wasm.js';

// --- STATE ---
let wallet = null;
let currentNet = "Testnet";
const NETS = { "Testnet": "http://localhost:3000/api", "Localhost": "http://127.0.0.1:3000/api" };

// --- ICONS ---
const ICONS = {
    settings: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>`,
    back: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"></line><polyline points="12 19 5 12 12 5"></polyline></svg>`
};

// --- DOM ---
const $ = (id) => document.getElementById(id);

// --- ROUTER ---
const router = {
    go(viewId, title = "Wallet") {
        document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
        $(viewId).classList.add('active');
        $('pageTitle').innerText = title;

        const btn = $('headerLeft');
        if (viewId === 'walletView') {
            btn.innerHTML = ICONS.settings;
            btn.onclick = () => router.go('settingsView', 'Settings');
        } else if (viewId === 'welcomeView') {
            btn.innerHTML = '';
        } else {
            btn.innerHTML = ICONS.back;
            btn.onclick = () => router.go('walletView'); // Default back to home
            if (viewId === 'networkView' || viewId === 'revealView') btn.onclick = () => router.go('settingsView', 'Settings');
        }
    }
};

// --- LOGIC ---
document.addEventListener('DOMContentLoaded', async () => {
    await init();
    try {
        if (localStorage.getItem('qua_sk')) {
            wallet = WalletKeys.from_private(localStorage.getItem('qua_sk'));
            updateData();
            router.go('walletView');
        } else {
            router.go('welcomeView');
        }
    } catch { router.go('welcomeView'); }
});

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

// Actions
$('createWalletBtn').onclick = () => {
    wallet = new WalletKeys();
    localStorage.setItem('qua_sk', wallet.get_private_key_hex());
    updateData();
    router.go('walletView');
};

$('navSend').onclick = () => router.go('sendView', 'Send QUA');
$('navReceive').onclick = () => router.go('receiveView', 'Receive QUA');

// Send Flow
$('broadcastBtn').onclick = async () => {
    const btn = $('broadcastBtn');
    btn.innerText = "Signing...";
    await new Promise(r => setTimeout(r, 800));
    btn.innerText = "Sent!";
    btn.style.background = "#14F195";
    setTimeout(() => {
        btn.innerText = "Review";
        btn.style.background = "#00E599";
        router.go('walletView');
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
    router.go('revealView', 'Secret Key');
};
$('navNetwork').onclick = () => router.go('networkView', 'Network');
$('doLogout').onclick = () => { localStorage.clear(); location.reload(); };

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
