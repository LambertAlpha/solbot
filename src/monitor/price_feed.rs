//! 价格监控器
//! 
//! 本模块负责实时监控多个DEX的价格数据，是套利系统的基础数据源。
//! 提供高效的价格获取、缓存和分发功能。
//!
//! ## 核心功能
//! - 多DEX价格数据获取
//! - 实时价格缓存和更新
//! - 价格数据验证和过滤
//! - 价格变化事件通知
//! - 健康状态监控
//!
//! ## 监控特性
//! - **实时性**: 毫秒级价格更新
//! - **可靠性**: 多数据源备份和故障转移
//! - **准确性**: 价格验证和异常检测
//! - **效率**: 智能缓存和批量处理
//! - **扩展性**: 支持动态添加DEX和交易对
//!
//! ## 数据流
//! 1. DEX API → 价格获取器
//! 2. 价格验证器 → 数据清洗
//! 3. 价格缓存器 → 数据存储
//! 4. 事件发布器 → 数据分发
//! 5. 监控器 → 状态检查

use super::{MonitorEvent, EventPublisher, MetricsCollector};
use crate::dex::{DexInterface, Price};
use crate::utils::MonitorConfig;
use anyhow::Result;
use tokio::sync::RwLock;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{error, info, warn};
use serde::{Deserialize, Serialize};
use crate::monitor::MonitorStats;

/// 交易对信息
/// 
/// 定义要监控的交易对的基本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingPair {
    /// 交易对符号 (例如: "SOL/USDC")
    pub symbol: String,
    /// 输入token地址
    pub input_mint: Pubkey,
    /// 输出token地址
    pub output_mint: Pubkey,
    /// 交易金额 (用于价格查询)
    pub amount: u64,
}

impl TradingPair {
    pub fn new(symbol: String, input_mint: Pubkey, output_mint: Pubkey, amount: u64) -> Self {
        Self {
            symbol,
            input_mint,
            output_mint,
            amount,
        }
    }
}

/// 价格缓存项
/// 
/// 缓存的价格数据，包含价格信息和更新时间
#[derive(Debug, Clone)]
struct PriceCache {
    /// 价格数据
    price: Price,
    /// 最后更新时间戳
    last_updated: u64,
}

impl PriceCache {
    fn new(price: Price) -> Self {
        Self {
            price,
            last_updated: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
    
    fn is_stale(&self, max_staleness_ms: u64) -> bool {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        now - self.last_updated > max_staleness_ms
    }
}

/// 价格监控器
/// 
/// 负责监控多个DEX的实时价格数据
/// 
/// ## 主要功能
/// - 并发监控多个DEX
/// - 实时价格数据获取和缓存
/// - 价格变化事件发布
/// - 健康状态监控
/// - 性能统计收集
/// 
/// ## 性能特性
/// - 异步并发处理
/// - 智能缓存机制
/// - 批量数据更新
/// - 内存高效存储
pub struct PriceFeed {
    /// DEX客户端列表
    dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>>,
    /// 监控的交易对列表
    trading_pairs: Vec<TradingPair>,
    /// 配置
    config: MonitorConfig,
    /// 事件发布器
    event_publisher: Arc<dyn EventPublisher + Send + Sync>,
    /// 运行状态
    is_running: Arc<RwLock<bool>>,
    /// 统计信息
    stats: Arc<Mutex<MonitorStats>>,
    /// 指标收集器
    metrics: Arc<Mutex<MetricsCollector>>,
    /// 价格缓存 (dex_name:symbol -> PriceCache)
    price_cache: Arc<RwLock<HashMap<String, PriceCache>>>,
    /// 停止信号发送器
    stop_sender: Arc<Mutex<Option<tokio::sync::broadcast::Sender<()>>>>,
    /// 监控任务
    monitoring_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// 健康检查结果
/// 
/// DEX健康状态检查的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// DEX名称
    pub dex_name: String,
    /// 是否健康
    pub is_healthy: bool,
    /// 响应时间 (毫秒)
    pub response_time_ms: u64,
    /// 最后成功时间
    pub last_success_time: Option<u64>,
    /// 错误信息 (如果不健康)
    pub error_message: Option<String>,
    /// 连续失败次数
    pub consecutive_failures: u32,
}

impl PriceFeed {
    /// 创建新的价格监控器
    pub fn new(
        dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>>,
        trading_pairs: Vec<TradingPair>,
        config: MonitorConfig,
        event_publisher: Arc<dyn EventPublisher + Send + Sync>,
    ) -> Self {
        let (stop_sender, _) = tokio::sync::broadcast::channel(1);
        Self {
            dex_clients,
            trading_pairs,
            config,
            event_publisher,
            is_running: Arc::new(RwLock::new(false)),
            stats: Arc::new(Mutex::new(MonitorStats::default())),
            metrics: Arc::new(Mutex::new(MetricsCollector::new(1000))),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            stop_sender: Arc::new(Mutex::new(Some(stop_sender))),
            monitoring_task: Arc::new(Mutex::new(None)),
        }
    }
    
    /// 启动价格监控
    pub async fn start(&self) -> Result<()> {
        if *self.is_running.read().await {
            return Ok(());
        }
        let mut is_running = self.is_running.write().await;
        *is_running = true;
        drop(is_running);
        info!("Starting price feed monitoring...");
        let dex_clients = self.dex_clients.clone();
        let trading_pairs = self.trading_pairs.clone();
        let config = self.config.clone();
        let event_publisher = self.event_publisher.clone();
        let price_cache = self.price_cache.clone();
        let stats = self.stats.clone();
        let is_running_flag = self.is_running.clone();
        let monitoring_task = self.monitoring_task.clone();
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.price_update_interval_ms));
            while *is_running_flag.read().await {
                interval.tick().await;
                for pair in &trading_pairs {
                    for dex_client in &dex_clients {
                        match dex_client.get_price(&pair.input_mint, &pair.output_mint, pair.amount).await {
                            Ok(price) => {
                                let mut cache = price_cache.write().await;
                                let key = format!("{}:{}", dex_client.get_name(), pair.symbol);
                                cache.insert(key, PriceCache::new(price.clone()));
                                drop(cache);
                                let event = MonitorEvent::PriceUpdate {
                                    dex_name: dex_client.get_name().to_string(),
                                    symbol: pair.symbol.clone(),
                                    price: price.bid,
                                    timestamp: price.timestamp,
                                };
                                if let Err(e) = event_publisher.publish(event).await {
                                    warn!("Failed to publish price update event: {}", e);
                                }
                                let mut stats_guard = stats.lock().await;
                                stats_guard.update_price_update();
                                stats_guard.update_price_fetch(true, 0.0);
                                drop(stats_guard);
                            }
                            Err(e) => {
                                error!("Failed to get price from {} for {}: {}", dex_client.get_name(), pair.symbol, e);
                                let mut stats_guard = stats.lock().await;
                                stats_guard.update_price_fetch(false, 0.0);
                                drop(stats_guard);
            }
                        }
                    }
                }
            }
            info!("Price feed monitoring stopped");
        });
        let mut task_guard = monitoring_task.lock().await;
        *task_guard = Some(handle);
        Ok(())
    }
    
    /// 停止价格监控
    pub async fn stop(&self) -> Result<()> {
        if !*self.is_running.read().await {
            return Ok(());
        }
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        drop(is_running);
        let mut task_guard = self.monitoring_task.lock().await;
        if let Some(task) = task_guard.take() {
            task.abort();
            }
        info!("Price feed monitoring stopped");
        Ok(())
    }
    
    /// 检查是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
    
    /// 获取统计信息
    pub async fn get_stats(&self) -> MonitorStats {
        self.stats.lock().await.clone()
    }
    
    /// 获取最新价格
    pub async fn get_latest_price(&self, dex_name: &str, symbol: &str) -> Option<Price> {
        let cache = self.price_cache.read().await;
        let key = format!("{}:{}", dex_name, symbol);
        cache.get(&key).map(|c| c.price.clone())
    }
    
    /// 获取所有最新价格
    pub async fn get_all_latest_prices(&self) -> HashMap<String, Price> {
        let cache = self.price_cache.read().await;
        cache.iter().map(|(k, v)| (k.clone(), v.price.clone())).collect()
    }
    
    /// 更新价格缓存
    pub async fn update_price_cache(&self, dex_name: &str, symbol: &str, price: Price) {
        let mut cache = self.price_cache.write().await;
        let key = format!("{}:{}", dex_name, symbol);
        cache.insert(key, PriceCache::new(price));
    }
    
    /// 健康检查所有DEX
    pub async fn health_check_all(&self) -> HashMap<String, bool> {
        let mut health_status = HashMap::new();
        
        for dex_client in &self.dex_clients {
            let dex_name = dex_client.get_name().to_string();
            match dex_client.health_check().await {
                Ok(healthy) => {
                    health_status.insert(dex_name, healthy);
                }
                Err(e) => {
                    warn!("Health check failed for {}: {}", dex_name, e);
                    health_status.insert(dex_name, false);
                }
            }
        }
        
        health_status
    }
    
    /// 添加交易对
    pub fn add_trading_pair(&mut self, pair: TradingPair) {
        self.trading_pairs.push(pair);
    }
    
    /// 清理过期的价格缓存
    pub async fn cleanup_stale_prices(&self) {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let max_staleness = self.config.max_price_staleness_ms;
        let mut cache = self.price_cache.write().await;
        cache.retain(|_, v| now - v.last_updated < max_staleness);
    }
}

// 实现Clone trait以支持多线程
impl Clone for PriceFeed {
    fn clone(&self) -> Self {
        Self {
            dex_clients: self.dex_clients.clone(),
            trading_pairs: self.trading_pairs.clone(),
            config: self.config.clone(),
            event_publisher: self.event_publisher.clone(),
            is_running: self.is_running.clone(),
            stats: self.stats.clone(),
            metrics: self.metrics.clone(),
            price_cache: self.price_cache.clone(),
            stop_sender: self.stop_sender.clone(),
            monitoring_task: self.monitoring_task.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::Price;
    use crate::monitor::EventBus;
    use anyhow::anyhow;
    
    // 模拟DEX客户端
    struct MockDexClient {
        name: String,
        should_fail: bool,
        response_delay_ms: u64,
    }

    impl MockDexClient {
        fn new(name: String, should_fail: bool, response_delay_ms: u64) -> Self {
            Self { name, should_fail, response_delay_ms }
        }
    }
    
    #[async_trait::async_trait]
    impl DexInterface for MockDexClient {
        async fn get_price(&self, _input_mint: &Pubkey, _output_mint: &Pubkey, _amount: u64) -> Result<Price> {
            tokio::time::sleep(Duration::from_millis(self.response_delay_ms)).await;
            
            if self.should_fail {
                return Err(anyhow!("Mock failure"));
            }
            
            Ok(Price::new(
                self.name.clone(),
                "SOL/USDC".to_string(),
                100.0,
                100.1,
                1000000.0,
            ))
        }
        
        async fn get_quote(&self, _input_mint: &Pubkey, _output_mint: &Pubkey, _amount: u64) -> Result<crate::dex::TradeInfo> {
            unimplemented!()
        }
        
        async fn execute_trade(&self, _trade_info: &crate::dex::TradeInfo) -> Result<String> {
            unimplemented!()
        }
        
        fn get_name(&self) -> &str {
            &self.name
        }
        
        async fn health_check(&self) -> Result<bool> {
            Ok(!self.should_fail)
        }
    }
    
    #[tokio::test]
    async fn test_price_feed_creation() {
        let dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>> = vec![];
        let trading_pairs = vec![];
        let config = MonitorConfig::default();
        let event_bus = Arc::new(EventBus::new(100));
        
        let price_feed = PriceFeed::new(dex_clients, trading_pairs, config, event_bus);
        assert!(!price_feed.is_running().await);
    }
    
    #[tokio::test]
    async fn test_price_feed_start_stop() {
        let dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>> = vec![];
        let trading_pairs = vec![];
        let config = MonitorConfig::default();
        let event_bus = Arc::new(EventBus::new(100));
        
        let price_feed = PriceFeed::new(dex_clients, trading_pairs, config, event_bus);
        
        // 启动
        price_feed.start().await.unwrap();
        assert!(price_feed.is_running().await);
        
        // 停止
        price_feed.stop().await.unwrap();
        assert!(!price_feed.is_running().await);
    }
    
    #[tokio::test]
    async fn test_price_feed_stats() {
        let price_feed = PriceFeed::new(
            vec![],
            vec![],
            crate::utils::MonitorConfig::default(),
            Arc::new(EventBus::new(100)),
        );
        
        let stats = price_feed.get_stats().await;
        assert_eq!(stats.price_updates, 0);
        assert_eq!(stats.successful_trades, 0);
        assert_eq!(stats.failed_trades, 0);
    }
    
    #[tokio::test]
    async fn test_price_feed_latest_price() {
        let dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>> = vec![];
        let trading_pairs = vec![];
        let config = MonitorConfig::default();
        let event_bus = Arc::new(EventBus::new(100));
        
        let price_feed = PriceFeed::new(dex_clients, trading_pairs, config, event_bus);
        
        // 测试获取不存在的价格
        let price = price_feed.get_latest_price("test_dex", "SOL/USDC").await;
        assert!(price.is_none());
    }
    
    #[tokio::test]
    async fn test_price_feed_health_check() {
        let dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>> = vec![];
        let trading_pairs = vec![];
        let config = MonitorConfig::default();
        let event_bus = Arc::new(EventBus::new(100));
        
        let price_feed = PriceFeed::new(dex_clients, trading_pairs, config, event_bus);
        
        // 测试健康检查
        let health_status = price_feed.health_check_all().await;
        assert!(health_status.is_empty());
    }
    
    #[tokio::test]
    async fn test_price_feed_trading_pair_management() {
        let dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>> = vec![];
        let trading_pairs = vec![];
        let config = MonitorConfig::default();
        let event_bus = Arc::new(EventBus::new(100));
        
        let mut price_feed = PriceFeed::new(dex_clients, trading_pairs, config, event_bus);
        
        // 添加交易对
        let pair = TradingPair {
            symbol: "SOL/USDC".to_string(),
            input_mint: Pubkey::default(),
            output_mint: Pubkey::default(),
            amount: 1_000_000,
        };
        price_feed.add_trading_pair(pair);
        
        // 清理过期价格
        price_feed.cleanup_stale_prices().await;
    }
}
