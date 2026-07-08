import requests
import json
import time
import concurrent.futures
import sys

import os

# Parse node IPs from the TARGET_IPS environment variable, or fallback to localhost
target_ips_env = os.environ.get("TARGET_IPS", "")
if target_ips_env:
    # Build URLs for each IP
    NODES = [f"http://{ip.strip()}:8332" for ip in target_ips_env.split(",") if ip.strip()]
else:
    NODES = [
        "http://127.0.0.1:8332",
    ]

# Configure payload according to Quanta's actual RPC transaction format
def send_transaction(node_url, tx_data):
    headers = {'Content-Type': 'application/json'}
    payload = {
        "jsonrpc": "2.0",
        "method": "send_transaction",
        "params": [tx_data],
        "id": 1
    }
    
    try:
        response = requests.post(node_url, data=json.dumps(payload), headers=headers, timeout=5)
        return response.status_code == 200
    except requests.exceptions.RequestException:
        return False

def spam_node(node_url, count):
    success = 0
    start_time = time.time()
    
    for i in range(count):
        dummy_tx = {
            "sender": f"test_sender_{i}",
            "receiver": "test_receiver",
            "amount": 1,
            "signature": "dummy_sig"
        }
        if send_transaction(node_url, dummy_tx):
            success += 1
            
        if i % 100 == 0 and i > 0:
            print(f"[{node_url}] Sent {i} txs...")
            
    end_time = time.time()
    print(f"[{node_url}] Completed: {success}/{count} successful in {end_time - start_time:.2f} seconds.")
    return success

def main():
    if len(sys.argv) > 1:
        tx_count = int(sys.argv[1])
    else:
        tx_count = 1000

    print(f"Starting Flood Test: Sending {tx_count} transactions to {len(NODES)} nodes...")
    
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(NODES)*10) as executor:
        futures = []
        for node in NODES:
            for _ in range(10):
                futures.append(executor.submit(spam_node, node, tx_count // 10))
                
        concurrent.futures.wait(futures)
        
    print("Flood test completed.")

if __name__ == "__main__":
    main()
