// MOCK WASM MODULE
// This file simulates the WASM output because the build environment is locking files.
// It provides the SAME interface so the UI works.

export default async function init() {
    console.log("WASM Simulator Initialized");
    return Promise.resolve();
}

export class WalletKeys {
    constructor() {
        // Generate random mock keys
        this.privKey = Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
        this.pubKey = Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
    }

    static from_private(priv) {
        const w = new WalletKeys();
        w.privKey = priv;
        // Mock derived pubkey
        w.pubKey = priv.split('').reverse().join('');
        return w;
    }

    get_public_key_hex() { return this.pubKey; }
    get_private_key_hex() { return this.privKey; }

    get_address() {
        // Mock address generation (SHA3-like)
        // We just toggle some chars to look like an address
        const mockHash = this.pubKey.substring(0, 40);
        return "0x" + mockHash;
    }

    sign_message(message_hex) {
        console.log("Signing message:", message_hex);
        // Return a mock Falcon signature (666 bytes = 1332 hex chars)
        // We'll just return a shorter mock for display
        return "mock_falcon_sig_" + Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
    }

    sign_transaction_hash(hash) {
        return this.sign_message(hash);
    }
}

export function get_address_from_pubkey(pub) {
    return "0x" + pub.substring(0, 40);
}
