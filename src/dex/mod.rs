//! DEX接口层模块
//!
//! 提供统一的DEX交互接口，支持多个Solana生态DEX
//! 包括价格查询、交易执行等核心功能

pub mod jupiter;
pub mod orca;
pub mod raydium;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// 统一的价格信息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    /// DEX名称
    pub dex_name: String,
    /// 交易对符号 (例如: "SOL/USDC")
    pub symbol: String,
    /// 买入价格 (你能以此价格卖出token)
    pub bid: f64,
    /// 卖出价格 (你能以此价格买入token)
    pub ask: f64,
    /// 中间价格
    pub mid: f64,
    /// 流动性深度 (USD)
    pub liquidity: f64,
    /// 时间戳 (毫秒)
    pub timestamp: u64,
}

/// 交易信息结构
#[derive(Debug, Clone)]
pub struct TradeInfo {
    /// 输入token地址
    pub input_mint: Pubkey,
    /// 输出token地址  
    pub output_mint: Pubkey,
    /// 输入数量
    pub amount_in: u64,
    /// 预期输出数量
    pub amount_out: u64,
    /// 最小输出数量 (滑点保护)
    pub minimum_amount_out: u64,
    /// 交易路径
    pub route: Vec<String>,
}

/// DEX交互的统一trait
#[async_trait::async_trait]
pub trait DexInterface {
    /// 获取指定交易对的价格
    async fn get_price(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
    ) -> Result<Price>;

    /// 获取交易路径和预期输出
    async fn get_quote(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
    ) -> Result<TradeInfo>;

    /// 执行交易
    async fn execute_trade(&self, trade_info: &TradeInfo) -> Result<String>;

    /// 获取DEX名称
    fn get_name(&self) -> &str;

    /// 检查DEX是否可用
    async fn health_check(&self) -> Result<bool>;
}

/// 价格计算工具函数
impl Price {
    /// 创建新的价格实例
    pub fn new(dex_name: String, symbol: String, bid: f64, ask: f64, liquidity: f64) -> Self {
        let mid = (bid + ask) / 2.0;
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        Self {
            dex_name,
            symbol,
            bid,
            ask,
            mid,
            liquidity,
            timestamp,
        }
    }

    /// 计算买卖价差
    pub fn spread(&self) -> f64 {
        self.ask - self.bid
    }

    /// 计算价差百分比
    pub fn spread_percentage(&self) -> f64 {
        if self.mid > 0.0 {
            (self.spread() / self.mid) * 100.0
        } else {
            0.0
        }
    }

    /// 检查价格是否过期 (默认5秒)
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        now - self.timestamp > max_age_ms
    }
}
