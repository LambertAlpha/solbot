//! 套利机会检测器
//!
//! 本模块负责检测和分析套利机会，是套利系统的核心组件。
//! 通过实时监控多个DEX的价格差异，识别潜在的套利机会。
//!
//! ## 核心功能
//! - 实时价格差异分析
//! - 套利机会识别和评分
//! - 风险评估和过滤
//! - 机会优先级排序
//! - 历史机会追踪
//!
//! ## 检测算法
//! - **简单套利检测**: 两个DEX间的直接价格差
//! - **三角套利检测**: 通过三种代币的循环交易
//! - **统计套利检测**: 基于历史价格模式的套利
//! - **流动性套利检测**: 利用流动性不平衡的套利
//!
//! ## 风险控制
//! - 流动性风险评估
//! - 价格影响计算
//! - 交易对手风险评估
//! - 市场风险分析

use super::ArbitrageOpportunity;
use crate::dex::{DexInterface, Price};
use crate::monitor::price_feed::PriceFeed;
use crate::monitor::EventBus;
use anyhow::Result;
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use std::num::NonZero;
use tracing::{debug, warn};
use crate::utils::AppConfig;

/// 检测器配置
/// 
/// 配置套利检测器的各种参数和阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorConfig {
    /// 最小利润阈值 (百分比)
    pub min_profit_threshold: f64,
    /// 最大价格影响 (百分比)
    pub max_price_impact: f64,
    /// 最小流动性要求 (USD)
    pub min_liquidity: f64,
    /// 检测间隔 (毫秒)
    pub detection_interval_ms: u64,
    /// 价格缓存时间 (秒)
    pub price_cache_ttl_seconds: u64,
    /// 最大并发检测数
    pub max_concurrent_detections: usize,
    /// 监控配置
    pub monitor: crate::utils::MonitorConfig,
    /// 策略配置
    pub strategy: crate::utils::StrategyConfig,
}

/// 套利机会检测器
/// 
/// 负责检测和分析套利机会的主要组件
/// 
/// ## 工作原理
/// 1. 从价格监控器获取实时价格数据
/// 2. 计算不同DEX间的价格差异
/// 3. 评估套利机会的可行性和风险
/// 4. 过滤和排序套利机会
/// 5. 返回符合条件的套利机会列表
/// 
/// ## 性能优化
/// - 使用LRU缓存减少重复计算
/// - 异步并发处理多个交易对
/// - 智能过滤减少无效检测
/// - 批量处理提高效率
#[derive(Clone)]
pub struct ArbitrageDetector {
    /// 价格监控器
    price_feed: Arc<PriceFeed>,
    /// 价格缓存 (symbol -> Price)
    price_cache: Arc<RwLock<LruCache<String, Price>>>,
    /// 检测配置
    config: DetectorConfig,
    /// 检测统计信息
    stats: Arc<RwLock<DetectorStats>>,
    /// 最后检测时间
    last_detection_time: Arc<RwLock<u64>>,
}

/// 检测器统计信息
/// 
/// 记录检测器的运行统计和性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorStats {
    /// 总检测次数
    pub total_detections: u64,
    /// 发现的机会数量
    pub opportunities_found: u64,
    /// 平均检测时间 (毫秒)
    pub average_detection_time_ms: f64,
    /// 最后检测时间戳
    pub last_detection_timestamp: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 错误次数
    pub error_count: u64,
}

impl Default for DetectorStats {
    fn default() -> Self {
        Self {
            total_detections: 0,
            opportunities_found: 0,
            average_detection_time_ms: 0.0,
            last_detection_timestamp: 0,
            cache_hit_rate: 0.0,
            error_count: 0,
        }
    }
}

/// 套利机会评分
/// 
/// 对套利机会进行综合评分，用于优先级排序
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityScore {
    /// 利润率评分 (0-100)
    pub profit_score: f64,
    /// 流动性评分 (0-100)
    pub liquidity_score: f64,
    /// 风险评分 (0-100, 越低越好)
    pub risk_score: f64,
    /// 执行难度评分 (0-100, 越低越好)
    pub execution_difficulty_score: f64,
    /// 综合评分 (0-100)
    pub overall_score: f64,
}

/// 风险评估结果
/// 
/// 包含套利机会的详细风险评估信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// 总体风险评分 (0-1, 越低越安全)
    pub overall_score: f64,
    /// 风险因素列表
    pub risk_factors: Vec<RiskFactor>,
    /// 风险建议
    pub recommendation: RiskRecommendation,
}

/// 风险因素
/// 
/// 描述具体的风险因素和影响程度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    /// 风险类型
    pub risk_type: RiskType,
    /// 风险描述
    pub description: String,
    /// 风险评分 (0-1)
    pub score: f64,
    /// 缓解建议
    pub mitigation: String,
}

/// 风险类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskType {
    /// 流动性风险
    Liquidity,
    /// 价格影响风险
    PriceImpact,
    /// 交易对手风险
    Counterparty,
    /// 市场风险
    Market,
    /// 技术风险
    Technical,
}

/// 风险建议
/// 
/// 基于风险评估的建议操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskRecommendation {
    /// 建议执行
    Execute,
    /// 建议谨慎执行
    ExecuteWithCaution,
    /// 建议等待
    Wait,
    /// 建议放弃
    Abandon,
}

impl ArbitrageDetector {
    /// 创建新的套利检测器
    pub fn new(
        price_feed: Arc<PriceFeed>,
        config: DetectorConfig,
    ) -> Self {
        let price_cache = Arc::new(RwLock::new(LruCache::new(NonZero::new(1000).unwrap())));
        let stats = Arc::new(RwLock::new(DetectorStats::default()));
        let last_detection_time = Arc::new(RwLock::new(0));

        Self {
            price_feed,
            price_cache,
            config,
            stats,
            last_detection_time,
        }
    }

    /// 寻找套利机会
    pub async fn find_opportunities(&self) -> Result<Vec<ArbitrageOpportunity>> {
        let start_time = Instant::now();
        
        // 获取最新价格数据
        let prices = self.price_feed.get_all_latest_prices().await;
        
        let mut opportunities = Vec::new();
        
        // 按交易对分组价格
        let mut prices_by_symbol: HashMap<String, Vec<Price>> = HashMap::new();
        for (key, price) in prices {
            let symbol = price.symbol.clone();
            prices_by_symbol.entry(symbol).or_insert_with(Vec::new).push(price);
        }
        
        // 分析每个交易对的价格差异
        for (symbol, price_data) in prices_by_symbol.iter() {
            if let Some(opportunity) = self.analyze_price_differences(symbol, price_data).await? {
                opportunities.push(opportunity);
            }
        }
        
        // 过滤和排序机会
        opportunities = opportunities
            .into_iter()
            .filter(|opp| opp.profit_percentage > self.config.min_profit_threshold)
            .collect();
        
        opportunities.sort_by(|a, b| b.profit_percentage.partial_cmp(&a.profit_percentage).unwrap());
        
        // 更新统计信息
        let mut stats = self.stats.write();
        stats.total_detections += 1;
        stats.opportunities_found += opportunities.len() as u64;
        stats.average_detection_time_ms = 
            (stats.average_detection_time_ms * (stats.total_detections - 1) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_detections as f64;
        stats.last_detection_timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        Ok(opportunities)
    }

    async fn get_dex_prices(&self, dex_client: &Arc<dyn DexInterface + Send + Sync>) -> Result<Vec<Price>> {
        let mut prices = Vec::new();
        let dex_name = dex_client.get_name();
        
        // 获取监控的交易对价格
        for trading_pair in &self.config.monitor.enabled_dexs {
            // 这里需要根据实际的交易对配置来获取价格
            // 暂时使用模拟数据
            if let Ok(price) = self.get_simulated_price(dex_name, trading_pair).await {
                prices.push(price);
            }
        }
        
        Ok(prices)
    }

    async fn get_simulated_price(&self, dex_name: &str, symbol: &str) -> Result<Price> {
        // 模拟价格数据，实际实现中应该调用DEX API
        let mut rng = rand::thread_rng();
        let base_price = match symbol {
            "SOL/USDC" => 145.0,
            "ETH/USDC" => 3500.0,
            "BTC/USDC" => 65000.0,
            _ => 100.0,
        };
        
        let spread = rng.gen_range(0.001..0.01); // 0.1% - 1% 价差
        let bid = base_price * (1.0 - spread / 2.0);
        let ask = base_price * (1.0 + spread / 2.0);
        
        Ok(Price::new(
            dex_name.to_string(),
            symbol.to_string(),
            bid,
            ask,
            rng.gen_range(100000.0..1000000.0), // 流动性
        ))
    }

    fn group_prices_by_symbol(&self, all_prices: &HashMap<String, Vec<Price>>) -> HashMap<String, Vec<Price>> {
        let mut grouped = HashMap::new();
        
        for (_dex_name, prices) in all_prices {
            for price in prices {
                let symbol = price.symbol.clone();
                grouped.entry(symbol).or_insert_with(Vec::new).push(price.clone());
            }
        }
        
        grouped
    }

    async fn analyze_price_differences(&self, _symbol: &str, prices: &[Price]) -> Result<Option<ArbitrageOpportunity>> {
        if prices.len() < 2 {
            return Ok(None);
        }
        
        let mut best_opportunity = None;
        let mut max_profit = 0.0;
        
        // 比较所有DEX之间的价格差异
        for i in 0..prices.len() {
            for j in i + 1..prices.len() {
                let price1 = &prices[i];
                let price2 = &prices[j];
                
                // 计算套利机会
                let (buy_dex, sell_dex, buy_price, sell_price) = if price1.ask < price2.bid {
                    (price1.dex_name.clone(), price2.dex_name.clone(), price1.ask, price2.bid)
                } else if price2.ask < price1.bid {
                    (price2.dex_name.clone(), price1.dex_name.clone(), price2.ask, price1.bid)
                } else {
                    continue;
                };
                
                let profit_percentage = (sell_price - buy_price) / buy_price * 100.0;
                
                // 考虑交易费用和滑点
                let total_fees = self.calculate_total_fees(&buy_dex, &sell_dex).await?;
                let adjusted_profit = profit_percentage - total_fees - self.config.strategy.slippage_tolerance;
                
                if adjusted_profit > max_profit && adjusted_profit > self.config.strategy.min_profit_threshold {
                    max_profit = adjusted_profit;
                    
                    let opportunity = ArbitrageOpportunity {
                        id: self.generate_opportunity_id(),
                        buy_dex: buy_dex.clone(),
                        sell_dex: sell_dex.clone(),
                        symbol: price1.symbol.clone(),
                        input_mint: Pubkey::default(),
                        output_mint: Pubkey::default(),
                        buy_price,
                        sell_price,
                        profit_percentage: adjusted_profit,
                        estimated_profit: adjusted_profit * buy_price / 100.0,
                        max_trade_amount: self.calculate_max_trade_amount(buy_price).await?,
                        risk_score: self.calculate_risk_score(&buy_dex, &sell_dex).await?,
                        created_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    
                    best_opportunity = Some(opportunity);
                }
            }
        }
        
        Ok(best_opportunity)
    }

    async fn calculate_total_fees(&self, buy_dex: &str, sell_dex: &str) -> Result<f64> {
        // 模拟费用计算，实际实现中需要查询DEX的具体费用
        let base_fee = 0.3; // 0.3% 基础费用
        let dex_fee_multiplier = match (buy_dex, sell_dex) {
            ("raydium", _) | (_, "raydium") => 1.2,
            ("orca", _) | (_, "orca") => 1.0,
            ("jupiter", _) | (_, "jupiter") => 0.8,
            _ => 1.0,
        };
        
        Ok(base_fee * dex_fee_multiplier)
    }

    async fn calculate_max_trade_amount(&self, price: f64) -> Result<u64> {
        let max_amount_sol = self.config.strategy.max_trade_amount as f64 / 1_000_000_000.0;
        let max_amount_usd = max_amount_sol * price;
        
        // 考虑流动性限制
        let liquidity_limit = (max_amount_usd * 0.1) as u64; // 假设流动性为交易量的10%
        
        Ok(std::cmp::min(
            self.config.strategy.max_trade_amount,
            liquidity_limit,
        ))
    }

    async fn calculate_risk_score(&self, buy_dex: &str, sell_dex: &str) -> Result<f64> {
        let mut risk_score: f64 = 0.0;
        
        // DEX风险评分
        let dex_risk = match (buy_dex, sell_dex) {
            ("raydium", "raydium") => 0.1,
            ("orca", "orca") => 0.1,
            ("jupiter", "jupiter") => 0.1,
            _ => 0.3, // 跨DEX交易风险较高
        };
        risk_score += dex_risk;
        
        // 流动性风险
        risk_score += 0.2;
        
        // 价格波动风险
        risk_score += 0.1;
        
        Ok(risk_score.min(1.0))
    }

    fn generate_opportunity_id(&self) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let random: u32 = rand::random();
        format!("arb_{}_{}", timestamp, random)
    }

    pub async fn should_execute(&self, opportunity: &ArbitrageOpportunity) -> Result<bool> {
        // 检查风险评分
        if opportunity.risk_score > 0.7 {
            warn!("Opportunity {} rejected due to high risk score: {}", 
                  opportunity.id, opportunity.risk_score);
            return Ok(false);
        }
        
        // 检查利润率
        if opportunity.profit_percentage < self.config.min_profit_threshold {
            return Ok(false);
        }
        
        // 检查时间戳
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if now - opportunity.timestamp > self.config.detection_interval_ms / 1000 {
            debug!("Opportunity {} is too old", opportunity.id);
            return Ok(false);
        }
        
        Ok(true)
    }

    /// 获取总机会数量
    pub fn get_total_opportunities(&self) -> u64 {
        // 这里需要异步访问，暂时返回0
        // 实际实现中应该使用tokio::task::block_in_place
        0
    }

    pub async fn get_risk_assessment(&self, opportunity: &ArbitrageOpportunity) -> Result<RiskAssessment> {
        let liquidity_risk = self.assess_liquidity_risk(opportunity).await?;
        let price_impact_risk = self.assess_price_impact_risk(opportunity).await?;
        let counterparty_risk = self.assess_counterparty_risk(opportunity).await?;
        let market_risk = self.assess_market_risk(opportunity).await?;
        
        let overall_score = (liquidity_risk + price_impact_risk + counterparty_risk + market_risk) / 4.0;
        
        let recommendation = if overall_score < 0.3 {
            RiskRecommendation::Execute
        } else if overall_score < 0.6 {
            RiskRecommendation::ExecuteWithCaution
        } else if overall_score < 0.8 {
            RiskRecommendation::Wait
        } else {
            RiskRecommendation::Abandon
        };
        
        Ok(RiskAssessment {
            overall_score,
            risk_factors: vec![], // 简化实现
            recommendation,
        })
    }

    async fn assess_liquidity_risk(&self, _opportunity: &ArbitrageOpportunity) -> Result<f64> {
        // 模拟流动性风险评估
        Ok(0.2)
    }

    async fn assess_price_impact_risk(&self, opportunity: &ArbitrageOpportunity) -> Result<f64> {
        // 基于交易量评估价格影响风险
        let trade_amount = opportunity.max_trade_amount as f64;
        let volume = 1_000_000.0; // 模拟24小时交易量
        
        let impact = trade_amount / volume;
        Ok(impact.min(1.0))
    }

    async fn assess_counterparty_risk(&self, _opportunity: &ArbitrageOpportunity) -> Result<f64> {
        // 模拟交易对手风险
        Ok(0.1)
    }

    async fn assess_market_risk(&self, _opportunity: &ArbitrageOpportunity) -> Result<f64> {
        // 模拟市场风险
        Ok(0.15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::DexInterface;
    use async_trait::async_trait;
    use solana_sdk::pubkey::Pubkey;

    struct MockDexClient {
        name: String,
        price_offset: f64,
        should_fail: bool,
    }

    #[async_trait]
    impl DexInterface for MockDexClient {
        async fn get_price(&self, _input_mint: &Pubkey, _output_mint: &Pubkey, _amount: u64) -> Result<crate::dex::Price> {
            // 模拟价格数据
            let price = crate::dex::Price::new(
                self.name.clone(),
                "SOL/USDC".to_string(),
                100.0 + self.price_offset as f64,
                100.1 + self.price_offset as f64,
                1000000.0,
            );
            Ok(price)
        }

        async fn get_quote(&self, _input_mint: &Pubkey, _output_mint: &Pubkey, _amount: u64) -> Result<crate::dex::TradeInfo> {
            // 模拟报价数据
            let trade_info = crate::dex::TradeInfo {
                input_mint: *_input_mint,
                output_mint: *_output_mint,
                amount_in: _amount,
                amount_out: _amount,
                minimum_amount_out: _amount,
                route: vec![self.name.clone()],
            };
            Ok(trade_info)
        }

        async fn execute_trade(&self, _trade_info: &crate::dex::TradeInfo) -> Result<String> {
            // 模拟交易执行
            Ok(format!("mock_tx_{}", chrono::Utc::now().timestamp()))
        }

        fn get_name(&self) -> &str {
            &self.name
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(!self.should_fail)
        }
    }

    #[tokio::test]
    async fn test_arbitrage_detector_creation() {
        let price_feed = Arc::new(PriceFeed::new(
            vec![],
            vec![],
            crate::utils::MonitorConfig::default(),
            Arc::new(EventBus::new(100)),
        ));
        
        let config = DetectorConfig {
            min_profit_threshold: 0.5,
            max_price_impact: 2.0,
            min_liquidity: 1000.0,
            detection_interval_ms: 1000,
            price_cache_ttl_seconds: 60,
            max_concurrent_detections: 10,
            monitor: crate::utils::MonitorConfig::default(),
            strategy: crate::utils::StrategyConfig::default(),
        };
        
        let detector = ArbitrageDetector::new(price_feed, config);
        assert_eq!(detector.get_total_opportunities(), 0);
    }

    #[tokio::test]
    async fn test_opportunity_id_generation() {
        let price_feed = Arc::new(PriceFeed::new(
            vec![],
            vec![],
            crate::utils::MonitorConfig::default(),
            Arc::new(EventBus::new(100)),
        ));
        
        let config = DetectorConfig {
            min_profit_threshold: 0.5,
            max_price_impact: 2.0,
            min_liquidity: 1000.0,
            detection_interval_ms: 1000,
            price_cache_ttl_seconds: 60,
            max_concurrent_detections: 10,
            monitor: crate::utils::MonitorConfig::default(),
            strategy: crate::utils::StrategyConfig::default(),
        };
        
        let detector = ArbitrageDetector::new(price_feed, config);
        let id1 = detector.generate_opportunity_id();
        let id2 = detector.generate_opportunity_id();
        
        assert_ne!(id1, id2);
        assert!(id1.starts_with("arb_"));
    }
}
