# Production RPC Node Setup Guide
*Securely expose your Quanta Node API for Web Wallets and Explorers.*

This guide describes how to securely expose your Quanta Node API to the internet utilizing Docker Host Networking, an NGINX Reverse Proxy, and Let's Encrypt SSL.

## 1. Run the Node with Host Networking

To securely bind your node to your VPS's strict localhost (`127.0.0.1`) without breaking security protocols, run the Docker container with `--network host`. This drops standard port mappings and cleanly routes all processes directly onto the server logic.

```bash
docker stop quanta-node && docker rm quanta-node

docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

## 2. Install NGINX and Certbot

Install the web server and the SSL certificate generator.

```bash
sudo apt update
sudo apt install -y nginx certbot python3-certbot-nginx
```

## 3. Open Cloud Firewall Ports

**CRITICAL:** Ensure that TCP Ports `80` (HTTP) and `443` (HTTPS) are permitted both in your local Ubuntu firewall (`iptables`) AND your Cloud Provider's Web Dashboard (e.g., Oracle Cloud VCN Security Lists, AWS Security Groups).

```bash
# Ubuntu UFW Configuration
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
```

## 4. Configure the NGINX Reverse Proxy

Create an NGINX configuration file for your domain (e.g., `rpc.quantachain.org`).
This acts as a secure bouncer and safely overrides restrictive Rust CORS headers to allow browser extension wallets to connect directly.

```bash
sudo nano /etc/nginx/sites-available/rpc.quantachain.org
```

Paste the following template:

```nginx
server {
    listen 80;
    server_name rpc.quantachain.org; # CHANGE TO YOUR DOMAIN

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        
        # 1. Hide the restrictive Rust security headers
        proxy_hide_header Access-Control-Allow-Origin;
        proxy_hide_header Access-Control-Allow-Methods;
        proxy_hide_header Access-Control-Allow-Headers;

        # 2. Inject wide-open CORS headers so web wallets can connect
        add_header 'Access-Control-Allow-Origin' '*' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'Content-Type' always;
        
        # 3. Handle browser "Preflight" requests natively via NGINX
        if ($request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' '*' always;
            add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
            add_header 'Access-Control-Allow-Headers' 'Content-Type' always;
            add_header 'Content-Type' 'text/plain charset=UTF-8';
            add_header 'Content-Length' 0;
            return 204;
        }
    }
}
```

## 5. Enable the Configuration

Enable the newly created site and restart NGINX.

```bash
sudo ln -s /etc/nginx/sites-available/rpc.quantachain.org /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl restart nginx
```

## 6. Generate the SSL Certificate

Run Certbot to seamlessly encrypt your traffic with Let's Encrypt HTTPS.

```bash
# Important: ensure your Domain's A Record points to the Server IP first!
sudo certbot --nginx -d rpc.quantachain.org 
```

Once successful, your Node RPC URL will be fully authenticated and accessible at:
`https://rpc.quantachain.org`
