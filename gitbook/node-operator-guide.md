# Node Operator Guide

This guide covers production-grade node deployment on a VPS, including Docker setup, firewall configuration, and exposing the API securely over HTTPS with NGINX.

---

## 1. VPS Setup

### Recommended Specs

| Parameter | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 4 vCPUs | 8 vCPUs |
| RAM | 8 GB | 16 GB |
| Storage | 1 TB SSD | 2 TB NVMe |
| OS | Ubuntu 22.04 LTS | Ubuntu 22.04 LTS |
| Bandwidth | 50/20 Mbps | 100/50 Mbps |

Compatible cloud providers: Oracle Cloud (free tier available), AWS, Hetzner, DigitalOcean, Linode.

---

## 2. Install Docker

```bash
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER
newgrp docker
docker --version
```

---

## 3. Configure Firewall

Open the required ports on your VPS:

```bash
sudo ufw allow ssh
sudo ufw allow 8333/tcp    # P2P networking
sudo ufw allow 3000/tcp    # REST API (close this if using NGINX proxy)
sudo ufw allow 7782/tcp    # RPC (restrict to localhost in production)
sudo ufw --force enable
```

**Important**: Also open these ports in your **cloud provider's dashboard** (e.g., Oracle VCN Security Lists, AWS Security Groups). UFW alone is not enough if the cloud has its own firewall layer.

---

## 4. Run the Node

### Option A: Standard Port Mapping (most users)

```bash
docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 \
  -p 8333:8333 \
  -p 7782:7782 \
  -p 9090:9090 \
  -v ~/quanta_data:/home/quanta/quanta_data \
  -v ~/quanta_logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

### Option B: Host Networking (for nodes behind an NGINX proxy)

Host networking binds the node directly to the VPS IP without port mapping. Use this when running an NGINX reverse proxy.

```bash
docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  -v ~/quanta_logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

---

## 5. Verify the Node

```bash
docker logs quanta-node --tail 30 -f
docker exec -it quanta-node quanta status --rpc-port 7782
docker exec -it quanta-node quanta print_height --rpc-port 7782
```

---

## 6. Expose the API Over HTTPS (NGINX + Let's Encrypt)

If you want to expose `rpc.quantachain.org` (or your own domain) for web wallets and explorers to connect:

### Install NGINX and Certbot

```bash
sudo apt update
sudo apt install -y nginx certbot python3-certbot-nginx
```

### Configure NGINX Reverse Proxy

Create a config file:

```bash
sudo nano /etc/nginx/sites-available/rpc.yourdomain.org
```

Paste and adjust the domain name:

```nginx
server {
    listen 80;
    server_name rpc.yourdomain.org;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;

        # Replace restrictive Rust CORS headers
        proxy_hide_header Access-Control-Allow-Origin;
        proxy_hide_header Access-Control-Allow-Methods;
        proxy_hide_header Access-Control-Allow-Headers;

        # Allow browser wallets to connect
        add_header 'Access-Control-Allow-Origin' '*' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'Content-Type' always;

        # Handle browser preflight requests
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

### Enable the Site

```bash
sudo ln -s /etc/nginx/sites-available/rpc.yourdomain.org /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl restart nginx
```

### Generate SSL Certificate

Ensure your domain's DNS A record points to your server IP first.

```bash
sudo certbot --nginx -d rpc.yourdomain.org
```

Your node API will be accessible at `https://rpc.yourdomain.org`.

---

## 7. Monitoring with Prometheus

The node exports Prometheus-compatible metrics on port 9090.

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'quanta'
    static_configs:
      - targets: ['localhost:9090']
```

Available metrics: blockchain height, TPS, peer count, mining hashrate, mempool size, block validation time, signature cache hit rate.

---

## 8. Upgrading the Node

```bash
docker pull xd637/quanta-node:latest
docker stop quanta-node && docker rm quanta-node

# Re-run with the same volume mounts
docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

Your data directory persists across upgrades. Check the [release notes](release-notes.md) before upgrading to see if a testnet reset is required.

---

## 9. Disk Space Management

```bash
# Remove stopped containers and dangling images
docker system prune -f

# Remove all unused images (frees most space)
docker image prune -a -f

# Check current disk usage
df -h
du -sh ~/quanta_data
```

---

## 10. Node Modes

Configure in `quanta.toml`:

```toml
[node]
# archive = keep all blocks (default)
# pruned  = keep last N days only
# light   = headers only (planned)
mode = "archive"
prune_days = 30    # only used when mode = "pruned"
```

| Mode | Storage | Use Case |
|------|---------|---------|
| `archive` | 1 TB+ | Full nodes, explorers, miners |
| `pruned` | ~400 GB | Validators, light operators |
| `light` | ~1 GB | SPV clients (planned) |

---

## 11. Security Best Practices

- **Never expose port 7782** (RPC) to the internet — it is for localhost CLI use only
- **Restrict SSH** to your IP if possible: `ufw allow from YOUR_IP to any port 22`
- Use `--restart always` to recover from crashes automatically
- Back up your `quanta_data` directory regularly (it contains the full chain state)
- Keep your wallet files (`*.qua`, `*.json`) off the server — sign transactions locally

---

## Troubleshooting

**Node fails to connect to peers**

```bash
docker exec -it quanta-node quanta peers --rpc-port 7782
```

Ensure port 8333 is open in both UFW and your cloud provider's security group.

**API returns 502 Bad Gateway**

The node isn't running or isn't listening on port 3000. Check:
```bash
docker ps
docker logs quanta-node --tail 30
```

**Node keeps restarting**

```bash
docker logs quanta-node --tail 50
```

Look for panic messages or storage errors. If the data directory is corrupted, you may need to resync from genesis.
