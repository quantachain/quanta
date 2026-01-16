// Quanta Wallet Content Script
// Injects window.quanta provider for dApp integration

(function () {
    'use strict';

    // Create the Quanta provider
    const quantaProvider = {
        isQuanta: true,
        version: '1.0.0',

        // Connection state
        _connected: false,
        _account: null,
        _network: 'localhost',

        // Event listeners
        _listeners: {
            accountsChanged: [],
            networkChanged: [],
            connect: [],
            disconnect: [],
        },

        // Request connection to wallet
        async connect() {
            return new Promise((resolve, reject) => {
                // Send message to background script
                window.postMessage({
                    type: 'QUANTA_CONNECT_REQUEST',
                    target: 'quanta-background'
                }, '*');

                // Listen for response
                const handleResponse = (event) => {
                    if (event.data.type === 'QUANTA_CONNECT_RESPONSE') {
                        window.removeEventListener('message', handleResponse);

                        if (event.data.approved) {
                            this._connected = true;
                            this._account = event.data.account;
                            this._network = event.data.network;
                            this._emit('connect', { account: this._account });
                            resolve({ account: this._account, network: this._network });
                        } else {
                            reject(new Error('User rejected connection'));
                        }
                    }
                };

                window.addEventListener('message', handleResponse);

                // Timeout after 60 seconds
                setTimeout(() => {
                    window.removeEventListener('message', handleResponse);
                    reject(new Error('Connection request timeout'));
                }, 60000);
            });
        },

        // Disconnect from wallet
        disconnect() {
            this._connected = false;
            this._account = null;
            this._emit('disconnect');
        },

        // Get current account
        async getAccount() {
            if (!this._connected) {
                throw new Error('Not connected. Call connect() first.');
            }
            return this._account;
        },

        // Get current network
        async getNetwork() {
            return this._network;
        },

        // Get balance
        async getBalance(address) {
            if (!address && this._account) {
                address = this._account;
            }

            return new Promise((resolve, reject) => {
                window.postMessage({
                    type: 'QUANTA_GET_BALANCE',
                    target: 'quanta-background',
                    address
                }, '*');

                const handleResponse = (event) => {
                    if (event.data.type === 'QUANTA_BALANCE_RESPONSE') {
                        window.removeEventListener('message', handleResponse);
                        resolve(event.data.balance);
                    }
                };

                window.addEventListener('message', handleResponse);

                setTimeout(() => {
                    window.removeEventListener('message', handleResponse);
                    reject(new Error('Balance request timeout'));
                }, 10000);
            });
        },

        // Sign and send transaction
        async sendTransaction(to, amount) {
            if (!this._connected) {
                throw new Error('Not connected. Call connect() first.');
            }

            return new Promise((resolve, reject) => {
                window.postMessage({
                    type: 'QUANTA_SEND_TRANSACTION',
                    target: 'quanta-background',
                    transaction: {
                        from: this._account,
                        to,
                        amount
                    }
                }, '*');

                const handleResponse = (event) => {
                    if (event.data.type === 'QUANTA_TRANSACTION_RESPONSE') {
                        window.removeEventListener('message', handleResponse);

                        if (event.data.success) {
                            resolve(event.data.txHash);
                        } else {
                            reject(new Error(event.data.error || 'Transaction failed'));
                        }
                    }
                };

                window.addEventListener('message', handleResponse);

                setTimeout(() => {
                    window.removeEventListener('message', handleResponse);
                    reject(new Error('Transaction request timeout'));
                }, 120000);
            });
        },

        // Sign message
        async signMessage(message) {
            if (!this._connected) {
                throw new Error('Not connected. Call connect() first.');
            }

            return new Promise((resolve, reject) => {
                window.postMessage({
                    type: 'QUANTA_SIGN_MESSAGE',
                    target: 'quanta-background',
                    message
                }, '*');

                const handleResponse = (event) => {
                    if (event.data.type === 'QUANTA_SIGN_RESPONSE') {
                        window.removeEventListener('message', handleResponse);

                        if (event.data.success) {
                            resolve(event.data.signature);
                        } else {
                            reject(new Error(event.data.error || 'Signing failed'));
                        }
                    }
                };

                window.addEventListener('message', handleResponse);

                setTimeout(() => {
                    window.removeEventListener('message', handleResponse);
                    reject(new Error('Sign request timeout'));
                }, 60000);
            });
        },

        // Event listener management
        on(event, callback) {
            if (this._listeners[event]) {
                this._listeners[event].push(callback);
            }
        },

        off(event, callback) {
            if (this._listeners[event]) {
                this._listeners[event] = this._listeners[event].filter(cb => cb !== callback);
            }
        },

        _emit(event, data) {
            if (this._listeners[event]) {
                this._listeners[event].forEach(callback => {
                    try {
                        callback(data);
                    } catch (error) {
                        console.error('Error in event listener:', error);
                    }
                });
            }
        }
    };

    // Listen for account/network changes from extension
    window.addEventListener('message', (event) => {
        if (event.data.type === 'QUANTA_ACCOUNT_CHANGED') {
            quantaProvider._account = event.data.account;
            quantaProvider._emit('accountsChanged', [event.data.account]);
        } else if (event.data.type === 'QUANTA_NETWORK_CHANGED') {
            quantaProvider._network = event.data.network;
            quantaProvider._emit('networkChanged', event.data.network);
        }
    });

    // Inject provider into window
    Object.defineProperty(window, 'quanta', {
        value: quantaProvider,
        writable: false,
        configurable: false
    });

    // Announce provider availability
    window.dispatchEvent(new Event('quanta#initialized'));

    console.log('🔮 Quanta Wallet Provider Injected');
})();
