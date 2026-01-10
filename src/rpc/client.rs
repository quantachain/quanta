use super::types::*;
use std::error::Error;

pub struct RpcClient {
    url: String,
    client: reqwest::Client,
}

impl RpcClient {
    pub fn new(port: u16) -> Self {
        Self {
            url: format!("http://127.0.0.1:{}", port),
            client: reqwest::Client::new(),
        }
    }

    pub async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<JsonRpcResponse, Box<dyn Error>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await?;

        let rpc_response: JsonRpcResponse = response.json().await?;
        Ok(rpc_response)
    }

    pub async fn get_node_status(&self) -> Result<NodeStatus, Box<dyn Error>> {
        let response = self.call("node_status", serde_json::json!({})).await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        let status: NodeStatus = serde_json::from_value(response.result.unwrap())?;
        Ok(status)
    }

    pub async fn get_mining_status(&self) -> Result<MiningStatus, Box<dyn Error>> {
        let response = self.call("mining_status", serde_json::json!({})).await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        let status: MiningStatus = serde_json::from_value(response.result.unwrap())?;
        Ok(status)
    }

    pub async fn get_block(&self, height: u64) -> Result<BlockInfo, Box<dyn Error>> {
        let response = self
            .call("get_block", serde_json::json!({ "height": height }))
            .await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        let block: BlockInfo = serde_json::from_value(response.result.unwrap())?;
        Ok(block)
    }

    pub async fn get_balance(&self, address: &str) -> Result<serde_json::Value, Box<dyn Error>> {
        let response = self
            .call("get_balance", serde_json::json!({ "address": address }))
            .await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        Ok(response.result.unwrap())
    }

    pub async fn get_peers(&self) -> Result<Vec<PeerInfo>, Box<dyn Error>> {
        let response = self.call("get_peers", serde_json::json!({})).await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        let peers: Vec<PeerInfo> = serde_json::from_value(response.result.unwrap())?;
        Ok(peers)
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn Error>> {
        let response = self.call("shutdown", serde_json::json!({})).await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        Ok(())
    }

    pub async fn start_mining(&self, address: &str) -> Result<(), Box<dyn Error>> {
        let response = self
            .call("start_mining", serde_json::json!({ "address": address }))
            .await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        Ok(())
    }

    pub async fn stop_mining(&self) -> Result<(), Box<dyn Error>> {
        let response = self.call("stop_mining", serde_json::json!({})).await?;
        
        if let Some(error) = response.error {
            return Err(format!("RPC Error: {}", error.message).into());
        }

        Ok(())
    }
}
