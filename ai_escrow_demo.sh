#!/bin/bash
set -e

# ==============================================================================
# QUANTA AI-to-AI Escrow Demo (BFT Testnet)
# 
# This script demonstrates how an AI Agent (Employer) can trustlessly hire 
# another AI Agent (Worker) using a cryptographic Escrow smart template.
# ==============================================================================

# Use local node by default, but allow override
NODE_URL=${1:-"http://127.0.0.1:3000"}

# Ensure wallet binary exists
WALLET_BIN="cargo run --bin quanta-wallet --quiet --"
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Please run this script from the root of the quanta repository."
    exit 1
fi

echo "=================================================="
echo "🤖 QUANTA AI-to-AI Escrow Demonstration"
echo "🌐 Target Node: $NODE_URL"
echo "=================================================="
echo ""

# 1. Generate Wallets
echo "[1/5] Generating Agent Wallets..."
export QUANTA_WALLET_PASSWORD="testpassword123"

# Remove old test wallets if they exist
rm -f employer_agent.qua worker_agent.qua

$WALLET_BIN new-raw --file employer_agent.qua > /dev/null
EMPLOYER_ADDR=$($WALLET_BIN --wallet employer_agent.qua address | grep "Address:" | awk '{print $2}')

$WALLET_BIN new-raw --file worker_agent.qua > /dev/null
WORKER_ADDR=$($WALLET_BIN --wallet worker_agent.qua address | grep "Address:" | awk '{print $2}')

echo "  👔 Employer Agent Address: $EMPLOYER_ADDR"
echo "  👷 Worker Agent Address:   $WORKER_ADDR"
echo ""

# 2. Fund the Employer
echo "[2/5] Funding Employer Wallet..."
echo "To deploy an escrow, the Employer needs testnet QUA."
echo "Waiting for the employer to have a balance >= 10 QUA..."
echo "  👉 PLEASE FUND: $EMPLOYER_ADDR"
echo "  (You can use another wallet or a genesis validator to send 10 QUA to this address)"

while true; do
    BALANCE_OUTPUT=$($WALLET_BIN --wallet employer_agent.qua --node "$NODE_URL" balance 2>/dev/null || echo "0")
    # Extract just the numeric whole QUA amount (assuming output like "Balance: 10.000000 QUA")
    BALANCE=$(echo "$BALANCE_OUTPUT" | grep -oP 'Balance: \K[0-9]+' || echo "0")
    
    if [ "$BALANCE" -ge 10 ]; then
        echo "  ✅ Received funds! Current Balance: $BALANCE QUA"
        break
    fi
    echo -n "."
    sleep 5
done
echo ""

# 3. Setup Escrow Condition
echo "[3/5] Setting up Escrow Condition..."
# In a real scenario, the worker gives the employer a hash of the work preimage.
SECRET="quanta_ai_task_completed_9921"
# SHA3-256 of the secret
SECRET_HASH="e5c63e0f730d9152dedf3d99dad24ccbb5acef4b36ba392b3ef03c04a193613e"
SECRET_HEX="7175616e74615f61695f7461736b5f636f6d706c657465645f39393231"

echo "  🔒 Secret Word:  $SECRET"
echo "  🔑 Secret Hash:  $SECRET_HASH"
echo ""

# 4. Deploy Escrow
echo "[4/5] Employer Deploying Escrow Contract..."
# The employer locks 5 QUA for the worker
DEPLOY_OUT=$($WALLET_BIN --wallet employer_agent.qua --node "$NODE_URL" deploy-escrow --beneficiary "$WORKER_ADDR" --secret-hash "$SECRET_HASH" --amount 5.0)

# Extract contract address from output
CONTRACT_ADDR=$(echo "$DEPLOY_OUT" | grep -oP 'Contract Address: \K0xc_[a-f0-9]+')
echo "$DEPLOY_OUT"
echo "  📜 Contract Deployed at: $CONTRACT_ADDR"
echo "Waiting for block confirmation (30s)..."
sleep 30
echo ""

# 5. Worker Claims Escrow
echo "[5/5] Worker Completes Task and Claims Escrow..."
# Worker calls claim-escrow providing the preimage to unlock funds
echo "  Worker submitting secret: $SECRET"
CLAIM_OUT=$($WALLET_BIN --wallet worker_agent.qua --node "$NODE_URL" claim-escrow --contract "$CONTRACT_ADDR" --preimage "$SECRET_HEX")

echo "$CLAIM_OUT"
echo "Waiting for block confirmation (30s)..."
sleep 30

# 6. Verify Final Balance
echo "[6/5] Verifying Final Worker Balance..."
WORKER_FINAL_BAL=$($WALLET_BIN --wallet worker_agent.qua --node "$NODE_URL" balance)
echo "  👷 $WORKER_FINAL_BAL"

echo ""
echo "=================================================="
echo "🎉 AI-to-AI Escrow Demo Completed Successfully!"
echo "=================================================="
