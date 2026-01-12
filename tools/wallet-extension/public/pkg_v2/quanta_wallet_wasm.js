// QUANTA WALLET CORE (SIMULATION MODE)
// This module provides the interface for the Quanta Wallet.
// In a production build, this would be backed by a WASM file with Falcon-512.
// For development/UI-testing, this simulation provides instant feedback.

export default async function init() {
    console.log("%c Quanta Wallet Core Initialized ", "background: #3b82f6; color: white; padding: 4px; border-radius: 4px;");
    return Promise.resolve();
}

export class WalletKeys {
    constructor() {
        // Generate valid-looking random hex keys
        this.privKey = this._randomHex(1281); // Falcon SK size
        this.pubKey = this._randomHex(897);   // Falcon PK size
    }

    _randomHex(bytes) {
        return Array.from({ length: bytes }, () => Math.floor(Math.random() * 256).toString(16).padStart(2, '0')).join('');
    }

    static from_private(priv) {
        const w = new WalletKeys();
        w.privKey = priv;
        // Mock derived pubkey deterministically from priv (simple reverse for consistency in sim)
        w.pubKey = priv.split('').reverse().join('').substring(0, 1794);
        return w;
    }

    get_public_key_hex() { return this.pubKey; }
    get_private_key_hex() { return this.privKey; }

    get_address() {
        // Deterministic address simulation based on PubKey
        // In real app: SHA3-256(PubKey)[0..20]
        let hash = 0;
        for (let i = 0; i < 50; i++) {
            hash = ((hash << 5) - hash) + this.pubKey.charCodeAt(i);
            hash |= 0;
        }
        const simHash = Math.abs(hash).toString(16).padStart(40, '0');
        return "0x" + simHash.substring(0, 40);
    }

    sign_message(message_hex) {
        console.log("Signing message:", message_hex);
        // Mock Falcon-512 Signature (666 bytes)
        return "mock_falcon_sig_" + this._randomHex(600);
    }

    sign_transaction_hash(hash) {
        return this.sign_message(hash);
    }
}

export function get_address_from_pubkey(pub) {
    return "0x" + pub.substring(0, 40);
}
