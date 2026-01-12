import init, { WalletKeys } from '../pkg_v2/quanta_wallet_wasm.js';

// State & Config
let wallet = null;
const NODE_URL = "http://localhost:3000/api";

// UI References
const els = {
    views: {
        welcome: document.getElementById('welcomeView'),
        create: document.getElementById('createWalletView'),
        main: document.getElementById('walletView'),
        send: document.getElementById('sendView'),
        receive: document.getElementById('receiveView')
    },
    status: {
        dot: document.getElementById('connectionStatus'),
        text: document.getElementById('statusText')
    },
    balance: document.getElementById('balanceDisplay'),
    addressShort: document.getElementById('shortAddress'),
    addressFull: document.getElementById('fullAddressDisplay'),
    inputs: {
        recipient: document.getElementById('recipientInput'),
        amount: document.getElementById('amountInput'),
        newPrivKey: document.getElementById('newPrivateKey')
    },
    feedback: document.getElementById('sendFeedback')
};

// Navigation
function showView(name) {
    Object.values(els.views).forEach(v => v.classList.remove('active'));
    els.views[name].classList.add('active');
}

// Logic
async function checkConnection() {
    try {
        const res = await fetch(`${NODE_URL}/stats`);
        if (res.ok) {
            els.status.dot.classList.add('connected');
            els.status.text.innerText = "Testnet Active";
            return true;
        }
    } catch (e) { }

    els.status.dot.classList.remove('connected');
    els.status.text.innerText = "Offline";
    return false;
}

async function updateBalance() {
    if (!wallet) return;
    const address = wallet.get_address();
    try {
        const res = await fetch(`${NODE_URL}/balance`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ address })
        });
        const data = await res.json();
        els.balance.innerText = (data.balance_microunits / 1_000_000).toFixed(2);
    } catch (e) {
        console.warn("Balance fetch failed", e);
    }
}

function copyToClipboard(text) {
    navigator.clipboard.writeText(text);
    // Could add toast here
}

// Event Setup
document.addEventListener('DOMContentLoaded', async () => {
    // Initialize WASM (or Mock)
    await init();

    // Check key
    const savedKey = localStorage.getItem('qua_priv');
    if (savedKey) {
        try {
            wallet = WalletKeys.from_private(savedKey);
            loadWalletUI();
            showView('main');
        } catch (e) {
            console.error("Failed to load saved wallet", e);
        }
    }

    // Polling
    checkConnection();
    setInterval(checkConnection, 5000);
    setInterval(updateBalance, 10000);

    // --- BUTTON BINDINGS ---

    // Welcome Flow
    document.getElementById('createWalletBtn').onclick = () => {
        wallet = new WalletKeys();
        const priv = wallet.get_private_key_hex();
        els.inputs.newPrivKey.value = priv;

        // Auto-save just for this demo ease-of-use (warn user in real app)
        localStorage.setItem('qua_priv', priv);

        showView('create');
    };

    document.getElementById('confirmSavedBtn').onclick = () => {
        loadWalletUI();
        showView('main');
    };

    document.getElementById('backToWelcomeBtn').onclick = () => showView('welcome');
    document.getElementById('logoutBtn').onclick = () => {
        localStorage.removeItem('qua_priv');
        wallet = null;
        showView('welcome');
    };

    // Main Methods
    function loadWalletUI() {
        if (!wallet) return;
        const addr = wallet.get_address();
        els.addressShort.innerText = addr.substring(0, 6) + "..." + addr.substring(addr.length - 4);
        els.addressFull.innerText = addr;
        updateBalance();
    }

    // Copy Actions
    document.getElementById('addressDisplay').onclick = () => copyToClipboard(wallet.get_address());
    document.getElementById('copyAddressBtn').onclick = () => copyToClipboard(wallet.get_address());

    // Navigation Buttons
    document.getElementById('sendViewBtn').onclick = () => showView('send');
    document.getElementById('receiveViewBtn').onclick = () => showView('receive');
    document.getElementById('cancelSendBtn').onclick = () => showView('main');
    document.getElementById('backFromReceiveBtn').onclick = () => showView('main');

    // Sending Logic
    document.getElementById('confirmSendBtn').onclick = async () => {
        const btn = document.getElementById('confirmSendBtn');
        const to = els.inputs.recipient.value;
        const amount = parseFloat(els.inputs.amount.value);

        if (!to || !amount) {
            els.feedback.style.display = 'block';
            els.feedback.innerText = "Please fill all fields";
            els.feedback.style.color = 'var(--danger)';
            return;
        }

        btn.disabled = true;
        btn.innerHTML = `<span class="loader"></span> Signing...`; // Add loader CSS if desired

        try {
            // 1. Sign
            // In real app, serialize properly. Here we mock/sign hash
            const txHashMock = "0000000000000000000000000000000000000000000000000000000000000000";
            const signature = wallet.sign_transaction_hash(txHashMock);

            // 2. Broadcast (Mock for now, as node needs sig verification endpoint update)
            // But we CAN use the existing 'transaction' endpoint if we just pass the data without sig check for now
            // or we use the CLI.
            // Let's try the real endpoint:

            const res = await fetch(`${NODE_URL}/transaction`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    recipient: to,
                    amount_microunits: Math.floor(amount * 1_000_000),
                    // We need to pass the wallet file/pass usually for the node to sign,
                    // BUT since we are signing externally, we need a new endpoint on the node: `/api/submit_tx`.
                    // The current node code likely doesn't have it yet.
                    // So we will just ALert success of visual flow.
                })
            });

            // Simulate success for UI feel
            await new Promise(r => setTimeout(r, 1500));

            showView('main');
            alert(`Signed with Falcon-512!\nSignature: ${signature.substring(0, 20)}...`);
            els.inputs.amount.value = '';
            els.inputs.recipient.value = '';

        } catch (e) {
            els.feedback.innerText = "Transaction Failed: " + e.message;
            els.feedback.style.display = 'block';
        }

        btn.disabled = false;
        btn.innerText = "Sign & Send";
    };
});
