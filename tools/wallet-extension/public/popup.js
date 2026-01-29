// WASM will be loaded dynamically to avoid blocking UI
let WalletKeys = null;
let wasmReady = false;

// Quanta Blockchain API Service (inline to avoid module issues)
class QuantaAPI {
    constructor(baseURL = 'http://localhost:3000') {
        this.baseURL = baseURL;
        this.testnetURL = 'http://testnet.quantachain.org:3000';
        this.currentNetwork = 'localhost';
    }

    setNetwork(network) {
        this.currentNetwork = network;
        if (network === 'testnet') {
            this.baseURL = this.testnetURL;
        } else {
            this.baseURL = 'http://localhost:3000';
        }
    }

    async request(endpoint, options = {}) {
        const url = `${this.baseURL}${endpoint}`;

        try {
            const response = await fetch(url, {
                headers: {
                    'Content-Type': 'application/json',
                    ...options.headers,
                },
                ...options,
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            return await response.json();
        } catch (error) {
            console.error(`API Request failed: ${endpoint}`, error);
            throw error;
        }
    }

    async getBalance(address) {
        try {
            const data = await this.request('/api/balance', {
                method: 'POST',
                body: JSON.stringify({ address }),
            });
            return data.balance_microunits;
        } catch (error) {
            console.error('Failed to fetch balance:', error);
            return 0;
        }
    }

    async getStats() {
        try {
            return await this.request('/api/stats');
        } catch (error) {
            console.error('Failed to fetch stats:', error);
            return null;
        }
    }

    async healthCheck() {
        try {
            return await this.request('/health');
        } catch (error) {
            console.error('Health check failed:', error);
            return { status: 'offline' };
        }
    }

    microunitsToQUA(microunits) {
        return (microunits / 1_000_000).toFixed(6);
    }

    quaToMicrounits(qua) {
        return Math.floor(parseFloat(qua) * 1_000_000);
    }
}

// --- TOAST NOTIFICATION ---
function showToast(message, duration = 3000) {
    const toast = document.getElementById('toast');
    toast.innerText = message;
    toast.classList.add('show');

    setTimeout(() => {
        toast.classList.remove('show');
    }, duration);
}

// --- STATE ---
let wallet = null;
let tempPassword = null; // Store temporarily during creation
let encryptedVault = null;
let api = new QuantaAPI(); // Initialize API service
let currentNetwork = localStorage.getItem('network') || 'localhost';

// --- AUTO-LOCK TIMER ---
let autoLockMinutes = parseInt(localStorage.getItem('autoLockMinutes') || '15');
let autoLockTimer = null;

function updateAutoLockDisplay() {
    const minutes = autoLockMinutes;
    let displayText;

    if (minutes === 0) {
        displayText = "Never";
    } else if (minutes === 60) {
        displayText = "1 hr";
    } else if (minutes === 1) {
        displayText = "1 min";
    } else {
        displayText = `${minutes} min`;
    }

    if ($('autoLockDisplay')) {
        $('autoLockDisplay').innerText = displayText;
    }

    // Update checkmarks
    ['1', '5', '15', '30', '60', 'Never'].forEach(id => {
        const checkEl = $('check' + id);
        if (checkEl) {
            const itemMinutes = id === 'Never' ? 0 : parseInt(id);
            checkEl.innerText = itemMinutes === minutes ? '●' : '';
        }
    });
}

function resetAutoLockTimer() {
    if (autoLockTimer) {
        clearTimeout(autoLockTimer);
    }

    if (autoLockMinutes > 0 && wallet) {
        autoLockTimer = setTimeout(() => {
            // Lock wallet
            wallet = null;
            location.reload();
        }, autoLockMinutes * 60 * 1000);
    }
}

function setAutoLockTime(minutes) {
    autoLockMinutes = minutes;
    localStorage.setItem('autoLockMinutes', minutes.toString());
    updateAutoLockDisplay();
    resetAutoLockTimer();
}

// Track user activity to reset auto-lock timer
document.addEventListener('click', () => {
    if (wallet) resetAutoLockTimer();
});

document.addEventListener('keydown', () => {
    if (wallet) resetAutoLockTimer();
});

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
            $('headerRight').innerHTML = ''; // No right icon
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
    console.log('🔮 Quanta Wallet Loading...');

    // Load WASM dynamically (non-blocking)
    try {
        console.log('Loading WASM module...');
        const wasmModule = await import('./pkg/quanta_wallet_wasm.js');
        await wasmModule.default();
        WalletKeys = wasmModule.WalletKeys;
        wasmReady = true;
        console.log('✅ WASM loaded successfully!');
    } catch (error) {
        console.error('❌ WASM loading failed:', error);
        console.warn('⚠️ Wallet will run in limited mode');
        // Don't block - UI will still work
    }

    // Initialize network from storage
    api.setNetwork(currentNetwork);

    // Update network UI
    if (currentNetwork === 'testnet') {
        $('checkTestnet').style.opacity = 1;
        $('checkLocal').style.opacity = 0;
        $('currentNetLabel').innerText = "Testnet >";
    } else {
        $('checkTestnet').style.opacity = 0;
        $('checkLocal').style.opacity = 1;
        $('currentNetLabel').innerText = "Localhost >";
    }

    // Check if wallet exists
    const storedVault = localStorage.getItem('qua_vault');
    if (storedVault) {
        encryptedVault = JSON.parse(storedVault);
        router.replace('loginView', 'Unlock');
    } else {
        router.replace('onboardingView', 'Welcome');
    }

    // Setup all event handlers AFTER DOM is loaded
    setupEventHandlers();

    console.log('✅ Wallet UI initialized');
});

// --- SETUP ALL EVENT HANDLERS ---
function setupEventHandlers() {
    console.log('Setting up event handlers...');

    // --- ONBOARDING FLOW ---
    $('startSetupBtn').onclick = () => router.push('createPasswordView', 'Setup');
    $('importWalletBtn').onclick = () => showToast("Import feature coming soon!");

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
        // Check if WASM is ready
        if (!wasmReady || !WalletKeys) {
            showToast('Wallet module is still loading. Please wait...', 3000);
            return;
        }

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
    // Tabs removed


    $('navSend').onclick = () => router.push('sendView', 'Send QUA');

    $('navReceive').onclick = () => {
        router.push('receiveView', 'Receive QUA');
        // Generate QR code when opening receive view
        if (wallet) {
            setTimeout(() => generateQRCode(wallet.get_address()), 100);
        }
    };

    // Send Flow
    $('broadcastBtn').onclick = async () => {
        const recipient = $('recipientInput').value;
        const amount = $('amountInput').value;

        if (!recipient || !amount || parseFloat(amount) <= 0) {
            showToast("Please enter a valid recipient address and amount");
            return;
        }

        const btn = $('broadcastBtn');
        btn.innerText = "Signing...";
        btn.disabled = true;

        await new Promise(r => setTimeout(r, 800));
        // Here we would actually use wallet.sign_transaction_hash()
        btn.innerText = "Sent!";
        btn.style.background = "#FFFFFF";
        btn.style.color = "#000000";

        setTimeout(() => {
            btn.innerText = "Review";
            btn.style.background = "#FFFFFF";
            btn.style.color = "#000000";
            btn.disabled = false;
            // Clear inputs
            $('recipientInput').value = '';
            $('amountInput').value = '';
            router.pop(); // Go back to wallet
        }, 1200);
    };

    // Receive - Generate QR Code
    let qrCodeInstance = null;
    function generateQRCode(address) {
        const qrContainer = $('qrcode');
        // Clear previous QR code
        qrContainer.innerHTML = '';

        // Generate new QR code
        if (typeof QRCode !== 'undefined') {
            qrCodeInstance = new QRCode(qrContainer, {
                text: address,
                width: 200,
                height: 200,
                colorDark: "#000000",
                colorLight: "#ffffff",
                correctLevel: QRCode.CorrectLevel.H
            });
        }
    }

    // Copy Address
    $('copyBtn').onclick = () => {
        if (!wallet) return;
        const address = wallet.get_address();
        navigator.clipboard.writeText(address).then(() => {
            $('copyBtn').innerText = "Copied!";
            setTimeout(() => $('copyBtn').innerText = "Copy Address", 1500);
        }).catch(err => {
            console.error('Failed to copy:', err);
            showToast('Failed to copy address');
        });
    };

    // Address pill copy
    $('addrPill').onclick = () => {
        if (!wallet) return;
        const address = wallet.get_address();
        navigator.clipboard.writeText(address).then(() => {
            const originalText = $('addressDisplay').innerText;
            $('addressDisplay').innerText = "Copied!";
            setTimeout(() => {
                const addr = wallet.get_address();
                $('addressDisplay').innerText = addr.substring(0, 6) + "..." + addr.substring(addr.length - 4);
            }, 1500);
        });
    };

    // Settings - Main Menu
    $('navAccounts').onclick = () => router.push('accountsView', 'Manage Accounts');

    // Password Modal Functions
    let pendingRevealAction = null;

    function showPasswordModal(callback) {
        pendingRevealAction = callback;
        $('passwordModal').classList.add('active');
        $('confirmPasswordInput').value = '';
        $('passwordError').style.opacity = 0;
        $('confirmPasswordInput').focus();
    }

    function hidePasswordModal() {
        $('passwordModal').classList.remove('active');
        pendingRevealAction = null;
        $('confirmPasswordInput').value = '';
        $('passwordError').style.opacity = 0;
    }

    $('cancelPasswordBtn').onclick = hidePasswordModal;

    $('confirmPasswordBtn').onclick = async () => {
        const password = $('confirmPasswordInput').value;
        const btn = $('confirmPasswordBtn');

        if (!password) {
            $('passwordError').innerText = "Please enter your password";
            $('passwordError').style.opacity = 1;
            return;
        }

        btn.innerText = "Verifying...";
        btn.disabled = true;

        try {
            // Verify password by trying to decrypt the vault
            await decryptVault(encryptedVault, password);

            // Password correct
            hidePasswordModal();
            if (pendingRevealAction) {
                pendingRevealAction();
            }
        } catch (e) {
            // Password incorrect
            $('passwordError').innerText = "Incorrect password";
            $('passwordError').style.opacity = 1;
            $('confirmPasswordInput').value = '';
            $('confirmPasswordInput').focus();
        } finally {
            btn.innerText = "Confirm";
            btn.disabled = false;
        }
    };

    // Allow Enter key to confirm password
    $('confirmPasswordInput').addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            $('confirmPasswordBtn').click();
        }
    });

    // Reveal Secret Key (with password protection)
    $('navReveal').onclick = () => {
        showPasswordModal(() => {
            $('secretBox').style.filter = "blur(8px)";
            $('secretBox').innerText = "CLICK TO REVEAL";
            router.push('revealView', 'Secret Key');
        });
    };

    $('navNetwork').onclick = () => router.push('networkView', 'Network');

    $('navChangePassword').onclick = () => router.push('changePasswordView', 'Change Password');

    $('navQuantumInfo').onclick = () => {
        showToast("Quanta uses Falcon-512 post-quantum cryptography to protect your assets against future quantum computer attacks.", 4000);
    };

    $('navAbout').onclick = () => router.push('aboutView', 'About Quanta');

    $('navPreferences').onclick = () => router.push('preferencesView', 'Preferences');

    $('navDeveloper').onclick = () => router.push('developerView', 'Developer Settings');

    $('doLogout').onclick = () => {
        if (confirm("Are you sure you want to lock your wallet?")) {
            wallet = null;
            location.reload();
        }
    };

    // Accounts Page
    $('addAccountBtn').onclick = () => {
        showToast("Add Account - Coming soon in v1.1");
    };

    // Security & Privacy Page
    $('navChangePasswordDetail').onclick = () => {
        router.push('changePasswordView', 'Change Password');
    };

    $('navAutoLock').onclick = () => {
        router.push('autoLockView', 'Auto-Lock');
        updateAutoLockDisplay();
    };



    // Change Password Implementation
    $('changePasswordBtn').onclick = async () => {
        const currentPwd = $('currentPasswordInput').value;
        const newPwd = $('newPasswordChangeInput').value;
        const confirmPwd = $('confirmNewPasswordInput').value;
        const errorEl = $('changePasswordError');
        const btn = $('changePasswordBtn');

        // Validation
        if (!currentPwd || !newPwd || !confirmPwd) {
            errorEl.innerText = "Please fill all fields";
            errorEl.style.opacity = 1;
            return;
        }

        if (newPwd.length < 8) {
            errorEl.innerText = "New password must be at least 8 characters";
            errorEl.style.opacity = 1;
            return;
        }

        if (newPwd !== confirmPwd) {
            errorEl.innerText = "New passwords do not match";
            errorEl.style.opacity = 1;
            return;
        }

        btn.innerText = "Changing...";
        btn.disabled = true;
        errorEl.style.opacity = 0;

        try {
            // Verify current password
            const keys = await decryptVault(encryptedVault, currentPwd);

            // Re-encrypt with new password
            const newVault = await encryptVault(keys, newPwd);
            localStorage.setItem('qua_vault', JSON.stringify(newVault));
            encryptedVault = newVault;

            // Success
            showToast("Password changed successfully!");

            // Clear inputs
            $('currentPasswordInput').value = '';
            $('newPasswordChangeInput').value = '';
            $('confirmNewPasswordInput').value = '';

            router.pop();
        } catch (e) {
            errorEl.innerText = "Current password is incorrect";
            errorEl.style.opacity = 1;
            $('currentPasswordInput').value = '';
            $('currentPasswordInput').focus();
        } finally {
            btn.innerText = "Change Password";
            btn.disabled = false;
        }
    };



    function updateAutoLockDisplay() {
        const minutes = autoLockMinutes;
        let displayText;

        if (minutes === 0) {
            displayText = "Never >";
        } else if (minutes === 60) {
            displayText = "1 hour >";
        } else if (minutes === 1) {
            displayText = "1 minute >";
        } else {
            displayText = `${minutes} minutes >`;
        }

        if ($('autoLockDisplay')) {
            $('autoLockDisplay').innerText = displayText;
        }

        // Update checkmarks
        ['1', '5', '15', '30', '60', 'Never'].forEach(id => {
            const checkEl = $('check' + id);
            if (checkEl) {
                const itemMinutes = id === 'Never' ? 0 : parseInt(id);
                checkEl.innerText = itemMinutes === minutes ? '●' : '';
            }
        });
    }

    function resetAutoLockTimer() {
        if (autoLockTimer) {
            clearTimeout(autoLockTimer);
        }

        if (autoLockMinutes > 0) {
            autoLockTimer = setTimeout(() => {
                // Lock wallet
                wallet = null;
                location.reload();
            }, autoLockMinutes * 60 * 1000);
        }
    }

    function setAutoLockTime(minutes) {
        autoLockMinutes = minutes;
        localStorage.setItem('autoLockMinutes', minutes.toString());
        updateAutoLockDisplay();
        resetAutoLockTimer();
    }

    // Auto-lock timer options
    ['1', '5', '15', '30', '60'].forEach(time => {
        $('lockTime' + time).onclick = () => setAutoLockTime(parseInt(time));
    });

    $('lockTimeNever').onclick = () => setAutoLockTime(0);

    // Preferences Page
    $('navLanguage').onclick = () => {
        showToast("Display Language - Coming soon in v1.1");
    };

    $('navCurrency').onclick = () => {
        showToast("Currency - Coming soon in v1.1");
    };

    // Developer Settings Page
    $('navQuantaNetwork').onclick = () => router.push('networkView', 'Network');

    // About Quanta Page
    $('navTerms').onclick = () => {
        window.open('https://quantachain.org/terms', '_blank');
    };

    $('navPrivacy').onclick = () => {
        window.open('https://quantachain.org/privacy', '_blank');
    };

    $('navWebsite').onclick = () => {
        window.open('https://quantachain.org', '_blank');
    };

    // Reveal
    $('secretBox').onclick = () => {
        $('secretBox').style.filter = "none";
        $('secretBox').innerText = wallet.get_private_key_hex();
    };

    // Network switching
    $('selTestnet').onclick = () => {
        $('checkTestnet').style.opacity = 1;
        $('checkLocal').style.opacity = 0;
        $('currentNetLabel').innerText = "Testnet >";
        api.setNetwork('testnet');
        currentNetwork = 'testnet';
        localStorage.setItem('network', 'testnet');
        if (wallet) updateData();
        showToast('Switched to Testnet');
    };

    $('selLocal').onclick = () => {
        $('checkTestnet').style.opacity = 0;
        $('checkLocal').style.opacity = 1;
        $('currentNetLabel').innerText = "Localhost >";
        api.setNetwork('localhost');
        currentNetwork = 'localhost';
        localStorage.setItem('network', 'localhost');
        if (wallet) updateData();
        showToast('Switched to Localhost');
    };


    async function updateData() {
        if (!wallet) return;
        const addr = wallet.get_address();
        $('addressDisplay').innerText = addr.substring(0, 6) + "..." + addr.substring(addr.length - 4);
        $('fullAddress').innerText = addr;

        // Fetch real balance from blockchain
        try {
            const balanceMicrounits = await api.getBalance(addr);
            const balanceQUA = api.microunitsToQUA(balanceMicrounits);
            $('balanceDisplay').innerText = balanceQUA;
        } catch (error) {
            console.error('Failed to fetch balance:', error);
            $('balanceDisplay').innerText = "0.00";
            showToast('Failed to connect to blockchain', 3000);
        }

        // Initialize auto-lock timer
        updateAutoLockDisplay();
        resetAutoLockTimer();

        // Check network health
        checkNetworkHealth();
    }

    // Check network connectivity
    async function checkNetworkHealth() {
        try {
            const health = await api.healthCheck();
            if (health && health.chain_height !== undefined) {
                console.log('✅ Connected to Quanta network');
                const heightEl = $('blockHeightDisplay');
                if (heightEl) heightEl.innerText = health.chain_height;

                // Update indicator if we have one
                const indicator = document.querySelector('#networkHealthItem .activity-item-right div');
                if (indicator) indicator.style.background = 'var(--success)';
            }
        } catch (error) {
            console.warn('⚠️ Network connection issue:', error);
            const heightEl = $('blockHeightDisplay');
            if (heightEl) heightEl.innerText = "OFFLINE";
        }
    }

    // Track user activity to reset auto-lock timer
    document.addEventListener('click', () => {
        if (wallet) resetAutoLockTimer();
    });

    document.addEventListener('keydown', () => {
        if (wallet) resetAutoLockTimer();
    });


} // End of setupEventHandlers()
