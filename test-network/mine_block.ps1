# Script to automatically create a wallet and mine a block on Node 1 (Testnet)

$NodeContainer = "quanta-node-1"
$MinerValuesFile = "miner_wallet.qua"
$ApiUrl = "http://localhost:3001/api/mine"

Write-Host "1. Creating new wallet for miner inside container..."
# Generate wallet and capture output
$OutputLines = docker exec -e QUANTA_WALLET_PASSWORD=password123 $NodeContainer /usr/local/bin/quanta new_wallet --file /home/quanta/$MinerValuesFile 2>&1
$Output = $OutputLines -join "`n"

# Parse address using regex
if ($Output -match "Address: (0x[a-fA-F0-9]+)") {
    $Address = $matches[1]
    Write-Host "   Success! Wallet created."
    Write-Host "   Miner Address: $Address"
    
    Write-Host "2. Sending Mining Request to Node 1..."
    try {
        $Response = Invoke-RestMethod -Uri $ApiUrl -Method Post -ContentType "application/json" -Body "{`"miner_address`": `"$Address`"}"
        
        Write-Host "   Response:"
        Write-Host ($Response | Out-String)
        
        if ($Response.success) {
            Write-Host "   SUCCESS! Block Mined."
            Write-Host "   Block Index: $($Response.block_index)"
        }
    } catch {
        Write-Host "   API Error: $_"
    }
} else {
    Write-Host "   Failed to parse address from output."
    Write-Host $Output
}
