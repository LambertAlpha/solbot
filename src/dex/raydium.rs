//! Raydium DEX接口实现
//! 
//! Raydium是Solana生态领先的AMM DEX
//! 提供流动性挖矿和订单簿交易功能

use super::{DexInterface, Price, TradeInfo};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Raydium池信息
#[derive(Debug, Deserialize, Clone)]
struct RaydiumPoolInfo {
    #[serde(rename = "id")]
    pubkey: String,
    #[serde(rename = "baseMint")]
    base_mint: String,
    #[serde(rename = "quoteMint")]
    quote_mint: String,
    #[serde(rename = "baseReserve")]
    base_reserve: String,
    #[serde(rename = "quoteReserve")]
    quote_reserve: String,
    #[serde(rename = "feeRate")]
    fee_rate: f64,
}

/// Raydium报价响应
#[derive(Debug, Deserialize)]
struct RaydiumQuoteResponse {
    #[serde(rename = "inputAmount")]
    input_amount: String,
    #[serde(rename = "outputAmount")]
    output_amount: String,
    #[serde(rename = "minimumOutputAmount")]
    minimum_output_amount: String,
    route: Vec<String>,
}

pub struct RaydiumClient {
    client: Client,
    base_url: String,
    pools_cache: Arc<Mutex<HashMap<String, RaydiumPoolInfo>>>,
    cache_timestamp: Arc<Mutex<u64>>,
    cache_ttl: u64,
}

impl RaydiumClient {
    pub fn new() -> Self {
        let client = Client::new();
        
        Self {
            client,
            base_url: "https://api-v3.raydium.io/".to_string(),
            pools_cache: Arc::new(Mutex::new(HashMap::new())),
            cache_timestamp: Arc::new(Mutex::new(0)),
            cache_ttl: 300, // 5分钟缓存
        }
    }

    /// 设置自定义API端点
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
    
    /// 设置缓存TTL
    pub fn with_cache_ttl(mut self, ttl_seconds: u64) -> Self {
        self.cache_ttl = ttl_seconds;
        self
    }

    /// 检查缓存是否有效
    async fn is_cache_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp() as u64;
        let cache_timestamp = *self.cache_timestamp.lock().await;
        now - cache_timestamp < self.cache_ttl
    }
    
    /// 更新池信息缓存
    async fn update_pools_cache(&self) -> Result<()> {
        let url = format!("{}/main/pairs", self.base_url);
        
        log::debug!("Fetching Raydium pools from: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch Raydium pools: {}", e))?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Raydium API returned error: {}", response.status()));
        }
        
        let pools: Vec<RaydiumPoolInfo> = response.json().await
            .map_err(|e| anyhow!("Failed to parse Raydium pools: {}", e))?;
        
        // 更新缓存
        let mut cache = self.pools_cache.lock().await;
        cache.clear();
        for pool in pools {
            let key = format!("{}_{}", pool.base_mint, pool.quote_mint);
            cache.insert(key, pool);
        }
        
        *self.cache_timestamp.lock().await = chrono::Utc::now().timestamp() as u64;
        log::info!("Updated Raydium pools cache: {} pools", cache.len());
        
        Ok(())
    }

    /// 查找池信息
    async fn find_pool(&self, input_mint: &Pubkey, output_mint: &Pubkey) -> Result<RaydiumPoolInfo> {
        // 检查缓存是否需要更新
        if !self.is_cache_valid().await {
            self.update_pools_cache().await?;
        }
        
        let key1 = format!("{}_{}", input_mint, output_mint);
        let key2 = format!("{}_{}", output_mint, input_mint);
        
        let cache = self.pools_cache.lock().await;
        
        cache.get(&key1)
            .or_else(|| cache.get(&key2))
            .cloned()
            .ok_or_else(|| anyhow!("Pool not found for pair: {} / {}", input_mint, output_mint))
    }

    /// 计算AMM价格（简化实现）
    fn calculate_amm_price(&self, pool: &RaydiumPoolInfo, input_mint: &Pubkey, amount: u64) -> Result<(f64, u64)> {
        // 简化的价格计算，实际实现需要更复杂的AMM公式
        let base_reserve: u64 = pool.base_reserve.parse().unwrap_or(0);
        let quote_reserve: u64 = pool.quote_reserve.parse().unwrap_or(0);
        
        if base_reserve == 0 || quote_reserve == 0 {
            return Err(anyhow!("Invalid pool reserves"));
        }
        
        let price = if pool.base_mint == input_mint.to_string() {
            quote_reserve as f64 / base_reserve as f64
        } else {
            base_reserve as f64 / quote_reserve as f64
        };
        
        let amount_out = (amount as f64 * price) as u64;
        Ok((price, amount_out))
    }

    /// 估算流动性
    fn estimate_liquidity(&self, pool: &RaydiumPoolInfo) -> Result<u64> {
        let base_reserve: u64 = pool.base_reserve.parse().unwrap_or(0);
        let quote_reserve: u64 = pool.quote_reserve.parse().unwrap_or(0);
        Ok(base_reserve + quote_reserve)
    }
}

#[async_trait::async_trait]
impl DexInterface for RaydiumClient {
    /// 获取价格信息
    async fn get_price(&self, input_mint: &Pubkey, output_mint: &Pubkey, amount: u64) -> Result<Price> {
        let pool = self.find_pool(input_mint, output_mint).await?;
        let (price, _) = self.calculate_amm_price(&pool, input_mint, amount)?;
        let liquidity = self.estimate_liquidity(&pool)?;
        
        let symbol = format!("{}/{}", input_mint, output_mint);
        Ok(Price::new(
            "raydium".to_string(),
            symbol,
            price * 0.999, // 买入价格（考虑滑点）
            price * 1.001, // 卖出价格（考虑滑点）
            liquidity as f64,
        ))
    }
    
    /// 获取交易报价
    async fn get_quote(&self, input_mint: &Pubkey, output_mint: &Pubkey, amount: u64) -> Result<TradeInfo> {
        let pool = self.find_pool(input_mint, output_mint).await?;
        let (_, amount_out) = self.calculate_amm_price(&pool, input_mint, amount)?;
        
        Ok(TradeInfo {
            input_mint: *input_mint,
            output_mint: *output_mint,
            amount_in: amount,
            amount_out,
            minimum_amount_out: (amount_out as f64 * 0.995) as u64, // 0.5% 滑点保护
            route: vec!["raydium".to_string()],
        })
    }
    
    /// 执行交易
    async fn execute_trade(&self, trade_info: &TradeInfo) -> Result<String> {
        // 模拟交易执行
        Ok("raydium_tx_".to_string() + &chrono::Utc::now().timestamp().to_string())
    }
    
    /// 获取DEX名称
    fn get_name(&self) -> &str {
        "raydium"
    }
    
    /// 健康检查
    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/main/info", self.base_url);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

impl Clone for RaydiumClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            base_url: self.base_url.clone(),
            pools_cache: self.pools_cache.clone(),
            cache_timestamp: self.cache_timestamp.clone(),
            cache_ttl: self.cache_ttl,
        }
    }
}
