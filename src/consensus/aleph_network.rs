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
    _phantom: std::marker::PhantomData<D>,
}

impl<D> QuantaNetworkBridge<D> {
    pub fn new(
        network: Arc<Network>,
        aleph_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Self {
        Self { 
            network, 
            aleph_rx: tokio::sync::Mutex::new(aleph_rx),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<D: Encode + Decode + Send + std::fmt::Debug + 'static> AlephNetwork<D> for QuantaNetworkBridge<D> {
    fn send(&self, data: D, _recipient: Recipient) {
        // Encode the AlephBFT NetworkData
        let encoded = data.encode();
        tracing::info!("AlephBFT attempting to broadcast a {} byte message to {:?}", encoded.len(), _recipient);
        
        let network = Arc::clone(&self.network);
        tokio::spawn(async move {
            network.broadcast_aleph_bft(encoded).await;
        });
    }

    async fn next_event(&mut self) -> Option<D> {
        tracing::info!("QuantaNetworkBridge: next_event() called by AlephBFT");
        let mut rx = self.aleph_rx.lock().await;
        while let Some(data) = rx.recv().await {
            tracing::info!("QuantaNetworkBridge: Dequeued {} bytes from aleph_rx. Attempting decode.", data.len());
            // Decode the data into D
            match D::decode(&mut &data[..]) {
                Ok(decoded) => {
                    tracing::info!("QuantaNetworkBridge: Successfully decoded {} bytes. Passing to AlephBFT: {:?}", data.len(), decoded);
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
