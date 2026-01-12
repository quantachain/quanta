#!/bin/bash
set -e

echo "=== QUANTA Testnet Node Initialization ==="
echo "API Port: ${API_PORT}"
echo "P2P Port: ${P2P_PORT}"
echo "RPC Port: ${RPC_PORT}"
echo "Bootstrap: ${BOOTSTRAP_PEER:-none}"
echo "Mining: ${ENABLE_MINING}"

# Data directory
DATA_DIR=${QUANTA_DB_PATH:-./quanta_data_testnet}
mkdir -p $DATA_DIR

# Create logs directory
mkdir -p logs

# Wallet creation
if [ ! -f "wallet.qua" ]; then
    echo "Creating new wallet..."
    # Set password environment variable for wallet creation
    export QUANTA_WALLET_PASSWORD="${QUANTA_WALLET_PASSWORD:-testnet_insecure_password}"
    /usr/local/bin/quanta new_wallet --file wallet.qua
    if [ $? -eq 0 ]; then
        echo "Wallet created successfully"
    else
        echo "ERROR: Failed to create wallet"
        exit 1
    fi
else
    echo "Using existing wallet: wallet.qua"
fi

# Print wallet info
echo ""
echo "=== Wallet Information ==="
export QUANTA_WALLET_PASSWORD="${QUANTA_WALLET_PASSWORD:-testnet_insecure_password}"
/usr/local/bin/quanta wallet --file wallet.qua --network testnet --db ${DATA_DIR} 2>&1 || echo "Warning: Could not display wallet info"
echo ""

# Start Node
echo "=== Starting QUANTA Node ==="
# Build command with optional bootstrap
CMD="/usr/local/bin/quanta start --config /home/quanta/server-config-testnet.toml --network testnet --port ${API_PORT} --network-port ${P2P_PORT} --rpc-port ${RPC_PORT} --db ${DATA_DIR}"

if [ ! -z "${BOOTSTRAP_PEER}" ]; then
    echo "Connecting to bootstrap peer: ${BOOTSTRAP_PEER}"
    CMD="$CMD --bootstrap ${BOOTSTRAP_PEER}"
else
    echo "Running as bootstrap node (no peers specified)"
fi

# CMD="$CMD --detach"  # Temporarily disabled for testing

echo "Executing: $CMD"
eval $CMD

# Wait for node to initialize
echo "Waiting for node to initialize..."
sleep 8

# Check if node is running
echo "Checking node status..."
/usr/local/bin/quanta status --rpc-port ${RPC_PORT} || {
    echo "WARNING: Node may not have started correctly"
}

# Start Mining if enabled
if [ "${ENABLE_MINING}" = "true" ]; then
    echo ""
    echo "=== Starting Mining ==="
    
    # Extract wallet address
    export QUANTA_WALLET_PASSWORD="${QUANTA_WALLET_PASSWORD:-testnet_insecure_password}"
    ADDRESS=$(/usr/local/bin/quanta wallet_address --file wallet.qua 2>/dev/null | grep -i "Address:" | awk '{print $NF}' | tr -d '\r\n')
    
    if [ ! -z "$ADDRESS" ]; then
        echo "Mining to address: $ADDRESS"
        /usr/local/bin/quanta start_mining "$ADDRESS" --rpc-port ${RPC_PORT} || {
            echo "ERROR: Failed to start mining"
        }
        echo "Mining started successfully"
    else
        echo "ERROR: Failed to extract wallet address for mining"
        echo "Attempting to show wallet info for debugging:"
        /usr/local/bin/quanta wallet --file wallet.qua --network testnet --db ${DATA_DIR} 2>&1 || true
    fi
else
    echo "Mining disabled for this node"
fi

echo ""
echo "=== Node Started Successfully ==="
echo "Following log file: logs/quanta_node_${P2P_PORT}.log"
echo ""

# Tail the log file to keep container alive and show output
LOG_FILE="logs/quanta_node_${P2P_PORT}.log"

# Wait for log file to be created
for i in {1..30}; do
    if [ -f "$LOG_FILE" ]; then
        echo "Log file found, streaming output..."
        break
    fi
    echo "Waiting for log file to be created... ($i/30)"
    sleep 1
done

if [ -f "$LOG_FILE" ]; then
    tail -f "$LOG_FILE"
else
    echo "ERROR: Log file not created at $LOG_FILE"
    echo "Keeping container alive anyway..."
    # Keep container running even if log file doesn't exist
    tail -f /dev/null
fi
