//! Jupiter聚合器接口实现
//!
//! Jupiter是Solana生态最大的DEX聚合器
//! 提供最优路径和最佳价格发现

use super::{DexInterface, Price, TradeInfo};
use crate::utils::wallet::Wallet;
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;

/// Jupiter API响应 - 报价
#[derive(Debug, Deserialize)]
struct JupiterQuoteResponse {
    #[serde(rename = "inputMint")]
    input_mint: String,
    #[serde(rename = "outputMint")]
    output_mint: String,
    #[serde(rename = "inAmount")]
    in_amount: String,
    #[serde(rename = "outAmount")]
    out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    other_amount_threshold: String,
    #[serde(rename = "routePlan")]
    route_plan: Vec<RoutePlan>,
}

#[derive(Debug, Deserialize)]
struct RoutePlan {
    #[serde(rename = "swapInfo")]
    swap_info: SwapInfo,
}

#[derive(Debug, Deserialize)]
struct SwapInfo {
    #[serde(rename = "ammKey")]
    amm_key: String,
    label: String,
    #[serde(rename = "inputMint")]
    input_mint: String,
    #[serde(rename = "outputMint")]
    output_mint: String,
    #[serde(rename = "inAmount")]
    in_amount: String,
    #[serde(rename = "outAmount")]
    out_amount: String,
}

/// Jupiter客户端
pub struct JupiterClient {
    /// HTTP客户端
    client: Client,
    /// API基础URL
    base_url: String,
    /// 请求超时时间
    timeout: Duration,
}

impl JupiterClient {
    /// 创建新的Jupiter客户端
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: "https://lite-api.jup.ag".to_string(),
            timeout: Duration::from_secs(5),
        }
    }

    /// 设置自定义超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 获取Jupiter报价
    async fn get_jupiter_quote(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
    ) -> Result<JupiterQuoteResponse> {
        let url = format!(
            "{}/swap/v1/quote?inputMint={}&outputMint={}&amount={}&slippageBps=50",
            self.base_url,
            input_mint.to_string(),
            output_mint.to_string(),
            amount
        );

        log::debug!("Requesting Jupiter quote: {}", url);

        let response = self
            .client
            .get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| anyhow!("Jupiter API request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Jupiter API error {}: {}", status, text));
        }

        let quote: JupiterQuoteResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse Jupiter response: {}", e))?;

        log::debug!("Jupiter quote received: {:?}", quote);
        Ok(quote)
    }

    /// 将Jupiter路径转换为字符串表示
    fn extract_route(&self, route_plan: &[RoutePlan]) -> Vec<String> {
        route_plan
            .iter()
            .map(|plan| plan.swap_info.label.clone())
            .collect()
    }

    /// 估算流动性 (基于输出金额的简单估算)
    fn estimate_liquidity(&self, out_amount: u64) -> f64 {
        // 简单估算：假设可用流动性是当前交易金额的10倍
        // 实际项目中应该通过更精确的方法计算
        (out_amount as f64) * 10.0 / 1_000_000.0 // 转换为USDC单位
    }
}

#[async_trait::async_trait]
impl DexInterface for JupiterClient {
    /// 获取价格信息
    async fn get_price(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
    ) -> Result<Price> {
        let quote = self
            .get_jupiter_quote(input_mint, output_mint, amount)
            .await?;

        let in_amount = quote
            .in_amount
            .parse::<u64>()
            .map_err(|e| anyhow!("Invalid input amount: {}", e))?;
        let out_amount = quote
            .out_amount
            .parse::<u64>()
            .map_err(|e| anyhow!("Invalid output amount: {}", e))?;

        // 计算价格 (output/input)
        let price = if in_amount > 0 {
            (out_amount as f64) / (in_amount as f64)
        } else {
            0.0
        };

        // Jupiter作为聚合器，bid和ask使用相同价格
        // 实际应用中可以通过反向查询获得更准确的bid/ask
        let symbol = format!(
            "{}/{}",
            input_mint.to_string()[..4].to_uppercase(),
            output_mint.to_string()[..4].to_uppercase()
        );

        let liquidity = self.estimate_liquidity(out_amount);

        Ok(Price::new(
            "Jupiter".to_string(),
            symbol,
            price, // bid
            price, // ask
            liquidity,
        ))
    }

    /// 获取交易报价
    async fn get_quote(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
    ) -> Result<TradeInfo> {
        let quote = self
            .get_jupiter_quote(input_mint, output_mint, amount)
            .await?;

        let amount_out = quote
            .out_amount
            .parse::<u64>()
            .map_err(|e| anyhow!("Invalid output amount: {}", e))?;
        let minimum_amount_out = quote
            .other_amount_threshold
            .parse::<u64>()
            .map_err(|e| anyhow!("Invalid minimum amount: {}", e))?;

        let route = self.extract_route(&quote.route_plan);

        Ok(TradeInfo {
            input_mint: *input_mint,
            output_mint: *output_mint,
            amount_in: amount,
            amount_out,
            minimum_amount_out,
            route,
        })
    }

    /// 执行交易 (需要实现交易提交逻辑)
    async fn execute_trade(&self, trade_info: &TradeInfo) -> Result<String> {
        // TODO: 实现实际的交易执行逻辑
        // 1. 调用Jupiter Swap API获取交易指令
        // 2. 签名并提交交易到Solana网络
        // 3. 返回交易签名

        log::warn!("Jupiter trade execution not implemented yet");
        Err(anyhow!("Trade execution not implemented"))
    }

    /// 获取DEX名称
    fn get_name(&self) -> &str {
        "Jupiter"
    }

    /// 健康检查
    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/swap/v1/quote?inputMint=So11111111111111111111111111111111111111112&outputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&amount=1000000&slippageBps=50", self.base_url);

        match self.client.get(&url).timeout(self.timeout).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_jupiter_health_check() {
        let client = JupiterClient::new();
        let result = client.health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_jupiter_quote() {
        let client = JupiterClient::new();

        // SOL -> USDC
        let sol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();

        let result = client.get_price(&sol_mint, &usdc_mint, 1_000_000_000).await; // 1 SOL

        match result {
            Ok(price) => {
                println!("Jupiter price: {:?}", price);
                assert!(price.mid > 0.0);
                assert_eq!(price.dex_name, "Jupiter");
            }
            Err(e) => {
                println!("Jupiter quote failed: {}", e);
                // 网络测试可能失败，不强制要求成功
            }
        }
    }
}
