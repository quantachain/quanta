#!/bin/bash
# QUANTA Testnet Deployment Script
# Deploy 3 nodes to production server

set -e  # Exit on error

SERVER_IP="207.148.73.146"
SERVER_USER="root"
DEPLOY_DIR="/opt/quanta"

echo "=========================================="
echo "QUANTA Testnet Deployment"
echo "Server: $SERVER_IP"
echo "Deploying 3 nodes"
echo "=========================================="

# Step 1: Install dependencies on server
echo ""
echo "Step 1: Installing dependencies..."
ssh $SERVER_USER@$SERVER_IP << 'ENDSSH'
    # Update system
    apt-get update
    apt-get upgrade -y
    
    # Install required packages
    apt-get install -y \
        curl \
        git \
        build-essential \
        pkg-config \
        libssl-dev \
        ufw \
        htop \
        nginx
    
    # Install Rust
    if ! command -v rustc &> /dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source $HOME/.cargo/env
    fi
    
    # Install Docker and Docker Compose
    if ! command -v docker &> /dev/null; then
        curl -fsSL https://get.docker.com -o get-docker.sh
        sh get-docker.sh
        rm get-docker.sh
    fi
    
    # Install Docker Compose
    if ! command -v docker-compose &> /dev/null; then
        curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
        chmod +x /usr/local/bin/docker-compose
    fi
    
    echo "Dependencies installed successfully!"
ENDSSH

# Step 2: Configure firewall
echo ""
echo "Step 2: Configuring firewall..."
ssh $SERVER_USER@$SERVER_IP << 'ENDSSH'
    # Enable UFW
    ufw --force enable
    
    # Allow SSH
    ufw allow 22/tcp
    
    # Allow HTTP/HTTPS (for API and web interface)
    ufw allow 80/tcp
    ufw allow 443/tcp
    
    # Allow QUANTA API ports (3000-3002)
    ufw allow 3000/tcp
    ufw allow 3001/tcp
    ufw allow 3002/tcp
    
    # Allow QUANTA P2P ports (8333-8335)
    ufw allow 8333/tcp
    ufw allow 8334/tcp
    ufw allow 8335/tcp
    
    # Allow RPC ports (localhost only - will be restricted by binding)
    # ufw allow from 127.0.0.1 to any port 7782
    # ufw allow from 127.0.0.1 to any port 7783
    # ufw allow from 127.0.0.1 to any port 7784
    
    # Allow Prometheus metrics (optional, can restrict to monitoring server)
    ufw allow 9090/tcp
    ufw allow 9091/tcp
    ufw allow 9092/tcp
    
    # Allow Grafana
    ufw allow 3030/tcp
    
    ufw status
    echo "Firewall configured successfully!"
ENDSSH

# Step 3: Create deployment directory
echo ""
echo "Step 3: Creating deployment directory..."
ssh $SERVER_USER@$SERVER_IP "mkdir -p $DEPLOY_DIR"

# Step 4: Copy project files to server
echo ""
echo "Step 4: Copying project files..."
rsync -avz --progress \
    --exclude 'target' \
    --exclude '.git' \
    --exclude 'node_modules' \
    --exclude '*.qua' \
    --exclude 'quanta_data*' \
    ./ $SERVER_USER@$SERVER_IP:$DEPLOY_DIR/

# Step 5: Build and start Docker containers
echo ""
echo "Step 5: Building and starting nodes..."
ssh $SERVER_USER@$SERVER_IP << ENDSSH
    cd $DEPLOY_DIR
    
    # Build Docker image
    docker-compose build
    
    # Start all services
    docker-compose up -d
    
    echo "Waiting for nodes to start..."
    sleep 30
    
    # Check status
    docker-compose ps
    
    echo ""
    echo "Checking node health..."
    curl -s http://localhost:3000/health || echo "Node 1 not ready yet"
    curl -s http://localhost:3001/health || echo "Node 2 not ready yet"
    curl -s http://localhost:3002/health || echo "Node 3 not ready yet"
ENDSSH

echo ""
echo "=========================================="
echo "Deployment Complete!"
echo "=========================================="
echo ""
echo "Your nodes are accessible at:"
echo "  - Node 1 API: http://$SERVER_IP:3000"
echo "  - Node 2 API: http://$SERVER_IP:3001"
echo "  - Node 3 API: http://$SERVER_IP:3002"
echo ""
echo "  - Node 1 API: http://api.testnet.quantachain.org"
echo "  - Node 2 API: http://testnet.quantachain.org:3001"
echo "  - Node 3 API: http://testnet.quantachain.org:3002"
echo ""
echo "  - Grafana Dashboard: http://$SERVER_IP:3030"
echo "    Username: admin"
echo "    Password: quanta2026"
echo ""
echo "Next steps:"
echo "  1. Check logs: ssh $SERVER_USER@$SERVER_IP 'cd $DEPLOY_DIR && docker-compose logs -f'"
echo "  2. Start mining: ssh $SERVER_USER@$SERVER_IP 'cd $DEPLOY_DIR && docker exec quanta-node2-miner quanta start_mining <ADDRESS>'"
echo "  3. Monitor metrics: http://$SERVER_IP:3030"
echo ""
