#!/bin/bash
# Simple QUANTA Deployment - Manual Build on Server
# This builds directly on the server without Docker

set -e

SERVER_IP="207.148.73.146"
SERVER_USER="root"
DEPLOY_DIR="/opt/quanta"

echo "=========================================="
echo "QUANTA Simple Deployment (No Docker)"
echo "=========================================="

# Step 1: Install Rust and dependencies
echo "Step 1: Installing dependencies..."
ssh $SERVER_USER@$SERVER_IP << 'ENDSSH'
    # Update system
    apt-get update
    apt-get install -y curl git build-essential pkg-config libssl-dev screen htop
    
    # Install Rust
    if ! command -v rustc &> /dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source $HOME/.cargo/env
    fi
    
    # Configure firewall
    apt-get install -y ufw
    ufw --force enable
    ufw allow 22/tcp
    ufw allow 80/tcp
    ufw allow 443/tcp
    ufw allow 3000/tcp
    ufw allow 3001/tcp
    ufw allow 3002/tcp
    ufw allow 8333/tcp
    ufw allow 8334/tcp
    ufw allow 8335/tcp
    ufw allow 9090/tcp
    ufw allow 9091/tcp
    ufw allow 9092/tcp
ENDSSH

# Step 2: Copy files
echo ""
echo "Step 2: Copying project files..."
ssh $SERVER_USER@$SERVER_IP "mkdir -p $DEPLOY_DIR"
rsync -avz --progress \
    --exclude 'target' \
    --exclude '.git' \
    --exclude '*.qua' \
    --exclude 'quanta_data*' \
    ./ $SERVER_USER@$SERVER_IP:$DEPLOY_DIR/

# Step 3: Build on server
echo ""
echo "Step 3: Building QUANTA..."
ssh $SERVER_USER@$SERVER_IP << 'ENDSSH'
    cd /opt/quanta
    source $HOME/.cargo/env
    cargo build --release
    
    # Create data directories
    mkdir -p quanta_data_node1
    mkdir -p quanta_data_node2
    mkdir -p quanta_data_node3
ENDSSH

# Step 4: Create systemd services
echo ""
echo "Step 4: Creating systemd services..."
ssh $SERVER_USER@$SERVER_IP << 'ENDSSH'
    # Node 1 service
    cat > /etc/systemd/system/quanta-node1.service << 'EOF'
[Unit]
Description=QUANTA Node 1 (Main)
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/quanta
ExecStart=/opt/quanta/target/release/quanta start -c /opt/quanta/quanta-node1.toml
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    # Node 2 service
    cat > /etc/systemd/system/quanta-node2.service << 'EOF'
[Unit]
Description=QUANTA Node 2 (Miner)
After=network.target quanta-node1.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/quanta
ExecStart=/opt/quanta/target/release/quanta start -c /opt/quanta/quanta-node2.toml
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    # Node 3 service
    cat > /etc/systemd/system/quanta-node3.service << 'EOF'
[Unit]
Description=QUANTA Node 3 (Peer)
After=network.target quanta-node1.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/quanta
ExecStart=/opt/quanta/target/release/quanta start -c /opt/quanta/quanta-node3.toml
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    # Reload systemd
    systemctl daemon-reload
    
    # Enable services
    systemctl enable quanta-node1
    systemctl enable quanta-node2
    systemctl enable quanta-node3
    
    # Start services
    systemctl start quanta-node1
    sleep 10
    systemctl start quanta-node2
    sleep 5
    systemctl start quanta-node3
    
    echo ""
    echo "Services started! Checking status..."
    systemctl status quanta-node1 --no-pager
    systemctl status quanta-node2 --no-pager
    systemctl status quanta-node3 --no-pager
ENDSSH

echo ""
echo "=========================================="
echo "Deployment Complete!"
echo "=========================================="
echo ""
echo "Check status:"
echo "  ssh $SERVER_USER@$SERVER_IP 'systemctl status quanta-node1'"
echo ""
echo "View logs:"
echo "  ssh $SERVER_USER@$SERVER_IP 'journalctl -u quanta-node1 -f'"
echo ""
echo "Test APIs:"
echo "  curl http://$SERVER_IP:3000/health"
echo "  curl http://$SERVER_IP:3001/health"
echo "  curl http://$SERVER_IP:3002/health"
echo ""
