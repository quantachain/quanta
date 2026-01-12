# Quanta Testnet Manual Verification Guide

This guide provides the commands to manually verify the Quanta Testnet fixes, including mining, synchronization, and node management.

## 1. Build and Start the Testnet
Rebuild the docker images and start the nodes in the background.

```powershell
docker-compose -f docker-compose.testnet.yml up --build -d
```

## 2. Get Mining Wallet Address
Retrieve the wallet address automatically generated inside the Node 1 container.

```powershell
docker exec -e QUANTA_WALLET_PASSWORD=testnet_insecure_password quanta-testnet-node1 /usr/local/bin/quanta wallet_address --file wallet.qua | Select-String "0x[a-fA-F0-9]{40}"
```

## 3. Start Mining
Start the miner on Node 1 using the address retrieved in the previous step.
**Important:** Replace `<WALLET_ADDRESS>` with the actual address from step 2.

```powershell
docker exec quanta-testnet-node1 /usr/local/bin/quanta start_mining <WALLET_ADDRESS> --rpc-port 17782
```

## 4. Verify Sync and Chain Growth
Check the chain height on both nodes. You should see the `chain_length` increasing on both nodes, confirming that Node 1 is mining and Node 2 is synchronizing.

**Single Check:**
```powershell
curl -s http://localhost:13000/api/stats
curl -s http://localhost:13001/api/stats
```

**Continuous Watch (Powershell):**
```powershell
while ($true) { 
    curl.exe -s http://localhost:13000/api/stats; 
    echo ""; 
    curl.exe -s http://localhost:13001/api/stats; 
    echo "---"; 
    Start-Sleep -Seconds 5 
}
```
*(Press Ctrl+C to stop the loop)*

## 5. Stop Mining
To stop the mining process on Node 1:

```powershell
docker exec quanta-testnet-node1 /usr/local/bin/quanta stop_mining --rpc-port 17782
```

## 6. Shutdown Testnet
To stop and remove the containers:

```powershell
docker-compose -f docker-compose.testnet.yml down
```

## 7. Shutdown and Wipe Data (Clean Start)
To stop containers and remove all blockchain data volumes for a fresh start:

```powershell
docker-compose -f docker-compose.testnet.yml down -v
```
