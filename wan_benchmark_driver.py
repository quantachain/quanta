import asyncio
import aiohttp
import time
import json
import statistics
import sys
from typing import List, Dict

# ==============================================================================
# QUANTA WAN Multi-Node Benchmark Driver
# 
# This script does NOT require the nodes to be perfectly clock-synced.
# It runs on a single "Driver" machine. It injects a transaction into one
# target node (e.g., US-East) and immediately starts polling the other
# remote nodes (e.g., London, Tokyo) to measure exactly how long it takes
# for the transaction to propagate through the P2P network and appear in their
# mempools or confirmed blocks.
# ==============================================================================

# Configure your 3-9 nodes here. 
# Node 0 will be the "Injector" (where we send the TX).
# The rest will be "Observers" (we poll them to see when the TX arrives).
NODES = [
    "http://127.0.0.1:3000", # Node 0 (Injector) - e.g., US East
    "http://127.0.0.1:3001", # Node 1 (Observer) - e.g., London
    "http://127.0.0.1:3002", # Node 2 (Observer) - e.g., Tokyo
]

async def check_tx_exists(session: aiohttp.ClientSession, node_url: str, tx_hash: str) -> bool:
    """Polls a node to see if a transaction has arrived in its mempool or block."""
    try:
        # Assuming the node has an endpoint to get a tx by hash
        # If not, you might need to poll /api/stats or /api/mempool
        async with session.get(f"{node_url}/api/transactions/{tx_hash}", timeout=1.0) as resp:
            return resp.status == 200
    except:
        return False

async def measure_propagation(session: aiohttp.ClientSession, tx_payload: dict) -> Dict[str, float]:
    """Injects to Node 0, then measures ms until it appears on Node 1..N"""
    injector_url = NODES[0]
    
    # 1. Inject the transaction
    start_time = time.perf_counter()
    async with session.post(f"{injector_url}/api/transactions/submit", json=tx_payload) as resp:
        if resp.status != 200:
            print(f"Injection failed: {resp.status}")
            return {}
        
    # Assume the TX payload has a pre-calculated hash or we can extract it.
    # If the API returns the hash, we'd extract it here. 
    # For now, let's assume we have `tx_payload['hash']`. 
    # (You will need to ensure your REST API returns the tx_hash or generate it locally).
    tx_hash = tx_payload.get("hash", "UNKNOWN_HASH") 
    
    # 2. Poll observers concurrently
    async def poll_node(node_url: str):
        while True:
            exists = await check_tx_exists(session, node_url, tx_hash)
            if exists:
                return time.perf_counter() - start_time
            await asyncio.sleep(0.05) # Poll every 50ms
            
            # Timeout after 10 seconds
            if time.perf_counter() - start_time > 10.0:
                return None

    tasks = [poll_node(node) for node in NODES[1:]]
    results = await asyncio.gather(*tasks)
    
    return {NODES[i+1]: (res * 1000 if res else None) for i, res in enumerate(results)}

async def main():
    print(f"Starting WAN Propagation Benchmark across {len(NODES)} nodes...")
    
    # Load your pre-signed transactions here
    # (Generate these using your Rust `quanta-wallet` or benchmark tool)
    try:
        with open("test_transactions.json", "r") as f:
            transactions = json.load(f)
    except FileNotFoundError:
        print("Please create 'test_transactions.json' containing a list of signed TXs.")
        sys.exit(1)

    latencies = {node: [] for node in NODES[1:]}

    async with aiohttp.ClientSession() as session:
        for i, tx in enumerate(transactions[:50]): # Test 50 txs
            print(f"[{i+1}/50] Injecting TX...")
            propagation_times = await measure_propagation(session, tx)
            
            for node, ms in propagation_times.items():
                if ms is not None:
                    latencies[node].append(ms)
                    print(f"  -> Arrived at {node} in {ms:.2f} ms")
                else:
                    print(f"  -> TIMEOUT at {node}")
                    
            await asyncio.sleep(1.0) # Wait a bit before next tx

    print("\n--- WAN Propagation Results ---")
    for node, times in latencies.items():
        if len(times) > 0:
            p50 = statistics.median(times)
            mean = statistics.mean(times)
            print(f"Node: {node}")
            print(f"  Deliveries: {len(times)}/50")
            print(f"  Median (p50): {p50:.2f} ms")
            print(f"  Mean:       {mean:.2f} ms")
        else:
            print(f"Node: {node} - No deliveries recorded.")

if __name__ == "__main__":
    asyncio.run(main())
