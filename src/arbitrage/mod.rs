//! 套利引擎模块
//!
//! 本模块实现了Solana DEX套利的核心逻辑，包括机会识别、风险评估和交易执行。
//! 支持多种套利策略，如简单套利、三角套利等。
//!
//! ## 核心组件
//! - **ArbitrageEngine**: 套利引擎主控制器
//! - **ArbitrageDetector**: 套利机会检测器
//! - **ArbitrageExecutor**: 交易执行器
//! - **ArbitrageStrategy**: 套利策略trait
//!
//! ## 套利策略类型
//! - **简单套利**: 两个DEX间的直接价格差套利
//! - **三角套利**: 通过三种代币的循环交易获利
//! - **跨链套利**: 利用不同链间的价格差异
//! - **流动性挖矿套利**: 结合流动性提供的收益优化
//!
//! ## 风险控制
//! - 滑点保护机制
//! - 最大损失限制
//! - 流动性风险评估
//! - 价格影响计算

pub mod detector;
pub mod executor;

pub use detector::ArbitrageDetector;
pub use executor::ArbitrageExecutor;

use crate::dex::DexInterface;
use crate::utils::AppConfig;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{info, warn};

use crate::dex::Price;
use crate::monitor::price_feed::PriceFeed;
use crate::utils::Wallet;

/// 套利机会信息
/// 
/// 描述一个具体的套利机会，包含买入/卖出DEX、价格信息、预期收益等
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// 唯一标识符
    pub id: String,
    /// 买入DEX名称
    pub buy_dex: String,
    /// 卖出DEX名称
    pub sell_dex: String,
    /// 交易对符号
    pub symbol: String,
    /// 买入价格
    pub buy_price: f64,
    /// 卖出价格
    pub sell_price: f64,
    /// 利润率百分比
    pub profit_percentage: f64,
    /// 最大交易金额
    pub max_trade_amount: u64,
    /// 预期收益
    pub estimated_profit: f64,
    /// 风险评分 (0-1, 越低越安全)
    pub risk_score: f64,
    /// 创建时间戳
    pub created_at: u64,
    /// 输入代币mint地址
    pub input_mint: Pubkey,
    /// 输出代币mint地址
    pub output_mint: Pubkey,
    /// 时间戳
    pub timestamp: u64,
}

/// 套利执行结果
/// 
/// 记录套利交易的执行结果和详细信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageResult {
    /// 交易签名
    pub signature: String,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 实际收益
    pub actual_profit: f64,
    /// 执行时间 (毫秒)
    pub execution_time_ms: u64,
    /// 错误信息 (如果失败)
    pub error_message: Option<String>,
    /// 交易费用
    pub fees: f64,
    /// 套利机会信息
    pub opportunity: Option<ArbitrageOpportunity>,
    /// 是否成功
    pub success: bool,
    /// 交易签名
    pub transaction_signature: Option<String>,
}

/// 执行状态枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// 执行成功
    Success,
    /// 执行失败
    Failed,
    /// 执行中
    Pending,
    /// 已取消
    Cancelled,
}

/// 套利策略trait
/// 
/// 定义套利策略的通用接口，支持自定义策略实现
#[async_trait::async_trait]
pub trait ArbitrageStrategy {
    /// 寻找套利机会
    async fn find_opportunities(&self) -> Result<Vec<ArbitrageOpportunity>>;
    
    /// 判断是否应该执行套利
    async fn should_execute(&self, opportunity: &ArbitrageOpportunity) -> Result<bool>;
    
    /// 计算最优交易金额
    async fn calculate_optimal_amount(&self, opportunity: &ArbitrageOpportunity) -> Result<u64>;
}

/// 套利引擎主控制器
/// 
/// 协调套利检测和执行，管理整个套利流程
/// 
/// ## 主要功能
/// - 启动和停止套利检测
/// - 管理套利机会队列
/// - 协调交易执行
/// - 收集统计信息
pub struct ArbitrageEngine {
    /// 套利检测器
    detector: ArbitrageDetector,
    /// 交易执行器
    executor: ArbitrageExecutor,
    /// 价格监控器
    price_feed: Arc<PriceFeed>,
    /// 钱包实例
    wallet: Arc<Wallet>,
    /// 配置信息
    config: AppConfig,
    /// 是否正在运行
    is_running: bool,
}

impl ArbitrageEngine {
    /// 创建新的套利引擎
    pub fn new(
        detector: ArbitrageDetector,
        executor: ArbitrageExecutor,
        price_feed: Arc<PriceFeed>,
        wallet: Arc<Wallet>,
        config: AppConfig,
    ) -> Self {
        Self {
            detector,
            executor,
            price_feed,
            wallet,
            config,
            is_running: true,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting arbitrage engine...");
        
        loop {
            match self.detector.find_opportunities().await {
                Ok(opportunities) => {
                    for opportunity in opportunities {
                        if self.detector.should_execute(&opportunity).await? {
                            info!("Executing arbitrage opportunity: {}", opportunity.id);
                            
                            match self.executor.execute_arbitrage(&opportunity).await {
                                Ok(result) => {
                                    if result.success {
                                        info!("Arbitrage executed successfully: {:?}", result);
                                    } else {
                                        warn!("Arbitrage execution failed: {:?}", result);
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to execute arbitrage: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to find arbitrage opportunities: {}", e);
                }
            }
            
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.monitor.price_update_interval_ms
            )).await;
        }
    }

    /// 获取统计信息
    pub async fn get_statistics(&self) -> Result<ArbitrageStatistics> {
        Ok(ArbitrageStatistics {
            total_detections: self.detector.get_total_opportunities(),
            opportunities_found: self.detector.get_total_opportunities(),
            successful_trades: self.executor.get_successful_trades(),
            failed_trades: self.executor.get_failed_trades(),
            total_profit: self.executor.get_total_profit(),
            total_loss: self.executor.get_total_loss(),
            average_execution_time_ms: self.executor.get_average_execution_time_ms(),
            success_rate: self.executor.get_success_rate(),
        })
    }
}

/// 套利统计信息
/// 
/// 记录套利引擎的运行统计和性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageStatistics {
    /// 总检测次数
    pub total_detections: u64,
    /// 发现的机会数量
    pub opportunities_found: u64,
    /// 成功执行的交易数量
    pub successful_trades: u64,
    /// 失败的交易数量
    pub failed_trades: u64,
    /// 总收益
    pub total_profit: f64,
    /// 总损失
    pub total_loss: f64,
    /// 平均执行时间 (毫秒)
    pub average_execution_time_ms: u64,
    /// 成功率百分比
    pub success_rate: f64,
}

impl Default for ArbitrageStatistics {
    fn default() -> Self {
        Self {
            total_detections: 0,
            opportunities_found: 0,
            successful_trades: 0,
            failed_trades: 0,
            total_profit: 0.0,
            total_loss: 0.0,
            average_execution_time_ms: 0,
            success_rate: 0.0,
        }
    }
}
