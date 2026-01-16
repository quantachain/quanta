// Quanta Blockchain API Service
// Connects to Quanta node for balance, transactions, and blockchain data

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

    // Get balance for an address
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

    // Get blockchain stats
    async getStats() {
        try {
            return await this.request('/api/stats');
        } catch (error) {
            console.error('Failed to fetch stats:', error);
            return null;
        }
    }

    // Get transaction history for an address
    async getTransactionHistory(address) {
        try {
            // Note: This endpoint needs to be implemented on the backend
            // For now, we'll return empty array
            return [];
        } catch (error) {
            console.error('Failed to fetch transaction history:', error);
            return [];
        }
    }

    // Submit a signed transaction
    async submitTransaction(signedTx) {
        try {
            const data = await this.request('/api/transaction/submit', {
                method: 'POST',
                body: JSON.stringify(signedTx),
            });
            return data;
        } catch (error) {
            console.error('Failed to submit transaction:', error);
            throw error;
        }
    }

    // Get mempool (pending transactions)
    async getMempool() {
        try {
            return await this.request('/api/mempool');
        } catch (error) {
            console.error('Failed to fetch mempool:', error);
            return { transaction_count: 0, transactions: [] };
        }
    }

    // Get specific block by height
    async getBlock(height) {
        try {
            return await this.request(`/api/block/${height}`);
        } catch (error) {
            console.error('Failed to fetch block:', error);
            return null;
        }
    }

    // Health check
    async healthCheck() {
        try {
            return await this.request('/health');
        } catch (error) {
            console.error('Health check failed:', error);
            return { status: 'offline' };
        }
    }

    // Get network peers
    async getPeers() {
        try {
            return await this.request('/api/peers');
        } catch (error) {
            console.error('Failed to fetch peers:', error);
            return { peer_count: 0, peers: [] };
        }
    }

    // Convert microunits to QUA (1 QUA = 1,000,000 microunits)
    microunitsToQUA(microunits) {
        return (microunits / 1_000_000).toFixed(6);
    }

    // Convert QUA to microunits
    quaToMicrounits(qua) {
        return Math.floor(parseFloat(qua) * 1_000_000);
    }
}

// Export for use in popup.js
export default QuantaAPI;
