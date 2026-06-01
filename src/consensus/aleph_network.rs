use codec::{Encode, Decode};
use aleph_bft::{Network as AlephNetwork, Recipient};
use tokio::sync::mpsc;
use async_trait::async_trait;
use std::sync::Arc;

use crate::network::Network;

/// Bridges AlephBFT's abstract network interface with Quanta's P2P network.
pub struct QuantaNetworkBridge<D> {
    /// Shared reference to the network manager for broadcasting messages.
    pub network: Arc<Network>,
    /// Channel to receive incoming BFT messages from Quanta's network manager.
    pub aleph_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<Vec<u8>>>,
    pub my_node_index: usize,
    _phantom: std::marker::PhantomData<D>,
}

impl<D> QuantaNetworkBridge<D> {
    pub fn new(
        network: Arc<Network>,
        aleph_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        my_node_index: usize,
    ) -> Self {
        Self { 
            network, 
            aleph_rx: tokio::sync::Mutex::new(aleph_rx),
            my_node_index,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<D: Encode + Decode + Send + std::fmt::Debug + 'static> AlephNetwork<D> for QuantaNetworkBridge<D> {
    fn send(&self, data: D, recipient: Recipient) {
        // Encode the AlephBFT NetworkData
        let mut encoded = Vec::new();
        match recipient {
            Recipient::Everyone => {
                encoded.push(0u8);
            }
            Recipient::Node(idx) => {
                encoded.push(1u8);
                encoded.extend_from_slice(&(idx.0 as u32).to_le_bytes());
            }
        }
        encoded.extend(data.encode());
        // tracing::info!("AlephBFT attempting to broadcast a {} byte message to {:?}", encoded.len(), recipient);
        
        let network = Arc::clone(&self.network);
        tokio::spawn(async move {
            network.broadcast_aleph_bft(encoded).await;
        });
    }

    async fn next_event(&mut self) -> Option<D> {
        let mut rx = self.aleph_rx.lock().await;
        while let Some(data) = rx.recv().await {
            if data.is_empty() { continue; }
            let r_type = data[0];
            let payload_start = if r_type == 0 { 1 } else { 5 };
            
            if r_type == 1 {
                if data.len() < 5 { continue; }
                let target_idx = u32::from_le_bytes(data[1..5].try_into().unwrap());
                if target_idx as usize != self.my_node_index {
                    continue; // intended for someone else
                }
            }
            
            if data.len() < payload_start { continue; }
            
            // Decode the data into D
            match D::decode(&mut &data[payload_start..]) {
                Ok(decoded) => {
                    return Some(decoded);
                }
                Err(e) => {
                    tracing::warn!("Failed to decode incoming AlephBFT message: {}", e);
                    continue;
                }
            }
        }
        None
    }
}
