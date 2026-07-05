use quanta::core::block::Block;
use quanta::core::transaction::StablecoinIntent;
use reqwest;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

const API_URL: &str = "http://127.0.0.1:3000/api/blocks/latest?count=10";

#[tokio::main]
async fn main() {
    println!("Starting Quanta Bridge Indexer Daemon...");
    println!("Monitoring for StablecoinIntents on Quanta Network...\n");

    let client = reqwest::Client::new();
    let mut last_processed_height: u64 = 0;

    loop {
        match fetch_latest_blocks(&client).await {
            Ok(blocks) => {
                // Blocks are returned in descending order (newest first).
                // Let's reverse them so we process from oldest to newest.
                let mut blocks_to_process = blocks.clone();
                blocks_to_process.sort_by_key(|b| b.index);

                for block in blocks_to_process {
                    if block.index > last_processed_height {
                        process_block(&block);
                        last_processed_height = block.index;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error fetching blocks: {}", e);
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
}

async fn fetch_latest_blocks(client: &reqwest::Client) -> Result<Vec<Block>, String> {
    let resp = client
        .get(API_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("API returned error status: {}", resp.status()));
    }

    let json: Value = resp.json().await.map_err(|e| e.to_string())?;

    if let Some(blocks_val) = json.get("blocks") {
        let blocks: Vec<Block> = serde_json::from_value(blocks_val.clone())
            .map_err(|e| format!("Failed to parse blocks: {}", e))?;
        Ok(blocks)
    } else {
        Err("Response missing 'blocks' field".to_string())
    }
}

fn process_block(block: &Block) {
    for tx in &block.transactions {
        // We only care about transactions that have a payload
        if tx.payload.is_empty() {
            continue;
        }

        // Attempt to parse the payload as a StablecoinIntent
        match StablecoinIntent::from_payload(&tx.payload) {
            Ok(intent) => {
                println!("[BRIDGE ALERT] Stablecoin Intent Detected!");
                println!("  Quanta TxHash: {}", tx.hash());
                println!("  Block Height : {}", block.index);
                println!(
                    "  Action       : Transfer {} {} to {} on {}",
                    intent.amount, intent.token, intent.recipient, intent.dest_chain
                );
                println!(
                    "  -- A bridge partner would execute this on {} now --\n",
                    intent.dest_chain
                );
            }
            Err(_) => {
                // Payload is not a StablecoinIntent, could be something else (e.g. Agent Job Hash)
                // We just ignore it
            }
        }
    }
}
