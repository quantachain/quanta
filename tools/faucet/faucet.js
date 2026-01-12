const express = require('express');
const { RateLimiterMemory } = require('rate-limiter-flexible');
const { exec } = require('child_process');
const path = require('path');
const fs = require('fs');

const app = express();
const port = 3000;

// Configuration
const QUANTA_BINARY = process.env.QUANTA_BINARY || 'quanta'; // Assumes 'quanta' is in PATH or specify full path
const FAUCET_WALLET = process.env.FAUCET_WALLET || 'faucet.qua';
const FAUCET_DB = process.env.FAUCET_DB || './quanta_data_testnet';
const FAUCET_PASSWORD = process.env.QUANTA_WALLET_PASSWORD || 'password'; // Needs to be set

// Rate Limiter: 1 request per 24 hours per IP
const rateLimiter = new RateLimiterMemory({
    points: 1,
    duration: 86400, // 24 hours
});

app.use(express.static('public')); // Serve frontend
app.use(express.json());

// API Endpoint
app.post('/api/faucet', async (req, res) => {
    const { address } = req.body;
    const ip = req.ip;

    if (!address) {
        return res.status(400).json({ success: false, error: 'Address is required' });
    }

    // Basic address validation (starts with '0x' or similar if applicable, length check)
    // Quanta addresses are usually longer (Falcon public keys or hashes).
    if (address.length < 10) {
        return res.status(400).json({ success: false, error: 'Invalid address format' });
    }

    try {
        await rateLimiter.consume(ip);

        console.log(`Sending 100 QUA to ${address}...`);

        // Construct command
        // Usage: quanta send --wallet <FILE> --to <ADDR> --amount <AMT> --db <DB>
        const cmd = `${QUANTA_BINARY} send --wallet "${FAUCET_WALLET}" --to "${address}" --amount 100 --db "${FAUCET_DB}"`;

        const env = { ...process.env, QUANTA_WALLET_PASSWORD: FAUCET_PASSWORD };

        exec(cmd, { env }, (error, stdout, stderr) => {
            if (error) {
                console.error(`Exec error: ${error}`);
                console.error(`Stderr: ${stderr}`);
                // If rate limit consumed but failed, ideally we shouldn't punish user, but keeping simple.
                return res.status(500).json({ success: false, error: 'Transaction failed. Faucet emptyor node issue.' });
            }

            console.log(`Success: ${stdout}`);
            // Parse TXID if possible, usually stdout contains "Transaction added... Nonce: ..."
            // We'll just return success.

            res.json({
                success: true,
                message: 'Sent 100 Testnet QUA',
                details: stdout.trim()
            });
        });

    } catch (rejRes) {
        res.status(429).json({ success: false, error: 'Rate limit exceeded. Try again in 24 hours.' });
    }
});

// Serve UI
app.get('/', (req, res) => {
    res.sendFile(path.join(__dirname, 'index.html'));
});

app.listen(port, () => {
    console.log(`Faucet running on http://localhost:${port}`);
    console.log(`Using Wallet: ${FAUCET_WALLET}`);
    console.log(`Using DB: ${FAUCET_DB}`);
});
