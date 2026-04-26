# Docker Deployment Guide

This guide covers production-grade Docker deployment for Quanta nodes on a VPS, including building and pushing custom images and exposing the RPC API securely over HTTPS.

---

## Running the Node (Quick)

```bash
docker pull xd637/quanta-node:latest

docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

---

## Exposing the RPC API Over HTTPS (Production)

If you run a public node or RPC endpoint (e.g., `rpc.quantachain.org`), use NGINX as a reverse proxy with Let's Encrypt SSL.

### 1. Run the Node with Host Networking

Host networking binds the node directly to the VPS network stack. Required for NGINX to proxy to `127.0.0.1:3000`.

```bash
docker stop quanta-node && docker rm quanta-node

docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

### 2. Install NGINX and Certbot

```bash
sudo apt update
sudo apt install -y nginx certbot python3-certbot-nginx
```

### 3. Open Firewall Ports

```bash
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow 8333/tcp
sudo ufw allow ssh
sudo ufw --force enable
```

Also open ports **80** and **443** in your cloud provider's dashboard (Oracle VCN, AWS Security Groups, etc.).

### 4. Configure NGINX

```bash
sudo nano /etc/nginx/sites-available/rpc.yourdomain.org
```

Paste the following (replace domain):

```nginx
server {
    listen 80;
    server_name rpc.yourdomain.org;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;

        # Replace restrictive Rust CORS headers with permissive ones
        # so browser extension wallets and web apps can connect
        proxy_hide_header Access-Control-Allow-Origin;
        proxy_hide_header Access-Control-Allow-Methods;
        proxy_hide_header Access-Control-Allow-Headers;

        add_header 'Access-Control-Allow-Origin' '*' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'Content-Type' always;

        # Handle browser preflight (OPTIONS) requests
        if ($request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' '*' always;
            add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
            add_header 'Access-Control-Allow-Headers' 'Content-Type' always;
            add_header 'Content-Length' 0;
            return 204;
        }
    }
}
```

### 5. Enable and Test

```bash
sudo ln -s /etc/nginx/sites-available/rpc.yourdomain.org /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl restart nginx
```

### 6. Issue SSL Certificate

Ensure your domain's DNS A record points to the server IP first.

```bash
sudo certbot --nginx -d rpc.yourdomain.org
```

Your node is now available at `https://rpc.yourdomain.org`. Certbot auto-renews the certificate.

---

## Building a Custom Docker Image

Use this workflow if you want to build and push a custom node image from your VPS.

### One-Time Setup

```bash
# Install Docker
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker ubuntu
newgrp docker

# Log in to Docker Hub
docker login -u YOUR_DOCKERHUB_USERNAME

# Clone the repo
git clone https://github.com/quantachain/quanta.git ~/quanta
```

### Build and Push (Each Release)

```bash
cd ~/quanta
git pull origin main

docker build \
  -t YOUR_USERNAME/quanta-node:v0.7.1-alpha \
  -t YOUR_USERNAME/quanta-node:latest \
  .

docker push YOUR_USERNAME/quanta-node:v0.7.1-alpha
docker push YOUR_USERNAME/quanta-node:latest
```

First build takes 10–15 minutes (downloads Rust crates). Subsequent builds reuse the cached layer and take ~5 minutes.

### Build in Background (SSH-safe)

```bash
# nohup — simple
nohup docker build \
  -t YOUR_USERNAME/quanta-node:v0.7.1-alpha \
  -t YOUR_USERNAME/quanta-node:latest \
  . > build.log 2>&1 &

tail -f build.log   # watch progress
```

Or use `screen` to detach and reattach later:

```bash
screen -S build
docker build -t YOUR_USERNAME/quanta-node:latest .
# Ctrl+A then D to detach; screen -r build to reattach
```

### Free Disk Space

After multiple builds, Docker can consume significant disk space:

```bash
docker system prune -f          # remove stopped containers and dangling images
docker image prune -a -f        # remove ALL unused images
```

---

## Upgrading the Node

```bash
docker pull xd637/quanta-node:latest
docker stop quanta-node && docker rm quanta-node

docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

Check the [release notes](release-notes.md) before upgrading. Most alpha releases are drop-in upgrades (no data wipe). The release notes will explicitly state when a testnet reset is required.
