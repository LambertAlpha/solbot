//! 监控模块
//! 
//! 本模块负责系统监控、事件管理和性能指标收集。
//! 提供实时监控、告警通知和统计数据分析功能。
//!
//! ## 核心组件
//! - **PriceFeed**: 价格监控器，负责实时价格数据获取
//! - **EventBus**: 事件总线，处理系统内部事件通信
//! - **MetricsCollector**: 指标收集器，收集性能数据
//! - **MonitorStats**: 监控统计信息
//!
//! ## 监控功能
//! - 实时价格监控和缓存
//! - 系统健康状态检查
//! - 性能指标收集和分析
//! - 异常事件检测和告警
//! - 数据统计和报告生成
//!
//! ## 事件类型
//! - 价格更新事件
//! - 套利机会发现事件
//! - 交易执行事件
//! - 系统错误事件
//! - 健康检查事件

pub mod price_feed;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::broadcast;

/// 监控事件类型
/// 
/// 定义系统中可能发生的各种事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitorEvent {
    /// 价格更新事件
    PriceUpdate {
        /// DEX名称
        dex_name: String,
        /// 交易对符号
        symbol: String,
        /// 新价格
        price: f64,
        /// 时间戳
        timestamp: u64,
    },
    /// 套利机会发现事件
    ArbitrageOpportunity {
        /// 机会ID
        opportunity_id: String,
        /// 预期收益
        expected_profit: f64,
        /// 风险评分
        risk_score: f64,
    },
    /// 交易执行事件
    TradeExecution {
        /// 交易签名
        signature: String,
        /// 执行状态
        success: bool,
        /// 实际收益
        profit: f64,
    },
    /// 系统错误事件
    SystemError {
        /// 错误类型
        error_type: String,
        /// 错误消息
        message: String,
        /// 严重程度
        severity: ErrorSeverity,
    },
    /// 健康检查事件
    HealthCheck {
        /// 检查的组件
        component: String,
        /// 检查结果
        healthy: bool,
        /// 响应时间 (毫秒)
        response_time_ms: u64,
    },
}

/// 错误严重程度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// 低严重程度 - 警告
    Low,
    /// 中等严重程度 - 需要注意
    Medium,
    /// 高严重程度 - 需要立即处理
    High,
    /// 严重程度 - 系统可能不可用
    Critical,
}

/// 监控统计信息
/// 
/// 记录系统监控的各项统计指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorStats {
    /// 总监控时间 (秒)
    pub uptime_seconds: u64,
    /// 价格更新次数
    pub price_updates: u64,
    /// 套利机会发现次数
    pub opportunities_found: u64,
    /// 成功执行的交易数
    pub successful_trades: u64,
    /// 失败的交易数
    pub failed_trades: u64,
    /// 系统错误次数
    pub errors: u64,
    /// 平均响应时间 (毫秒)
    pub average_response_time_ms: f64,
    /// 最后更新时间戳
    pub last_update: u64,
}

/// 事件发布者trait
/// 
/// 定义事件发布的标准接口，支持多种事件发布方式
#[async_trait::async_trait]
pub trait EventPublisher: Send + Sync {
    /// 发布事件
    async fn publish(&self, event: MonitorEvent) -> Result<()>;
    
    /// 订阅事件
    fn subscribe(&self) -> broadcast::Receiver<MonitorEvent>;
    
    /// 获取订阅者数量
    fn subscriber_count(&self) -> usize;
}

/// 事件总线实现
/// 
/// 基于tokio broadcast channel的事件总线，支持多订阅者模式
/// 
/// ## 特性
/// - 异步事件发布和订阅
/// - 支持多个订阅者
/// - 自动处理订阅者断开连接
/// - 内存高效的事件传递
pub struct EventBus {
    /// 事件发送器
    sender: broadcast::Sender<MonitorEvent>,
    /// 订阅者数量
    subscriber_count: usize,
}

/// 指标收集器
/// 
/// 收集和存储系统性能指标，支持历史数据查询和统计分析
/// 
/// ## 功能
/// - 实时指标收集
/// - 历史数据存储
/// - 统计分析计算
/// - 数据清理和压缩
pub struct MetricsCollector {
    /// 指标数据 (metric_name -> (timestamp, value))
    metrics: HashMap<String, Vec<(u64, f64)>>,
    /// 最大保留数据点数量
    max_data_points: usize,
}

impl Default for MonitorStats {
    fn default() -> Self {
        Self {
            uptime_seconds: 0,
            price_updates: 0,
            opportunities_found: 0,
            successful_trades: 0,
            failed_trades: 0,
            errors: 0,
            average_response_time_ms: 0.0,
            last_update: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
}

impl MonitorStats {
    /// 计算成功率
    pub fn success_rate(&self) -> f64 {
        let total = self.successful_trades + self.failed_trades;
        if total == 0 {
            1.0
        } else {
            self.successful_trades as f64 / total as f64
        }
    }
    
    /// 更新价格获取统计
    pub fn update_price_fetch(&mut self, success: bool, response_time_ms: f64) {
        if success {
            self.price_updates += 1;
        } else {
            self.errors += 1;
        }
        
        // 更新平均响应时间
        let total_fetches = self.price_updates + self.errors;
        if total_fetches > 0 {
            self.average_response_time_ms = 
                (self.average_response_time_ms * (total_fetches - 1) as f64 + response_time_ms) / total_fetches as f64;
        }
        
        self.last_update = chrono::Utc::now().timestamp_millis() as u64;
    }
    
    /// 更新价格更新统计
    pub fn update_price_update(&mut self) {
        self.price_updates += 1;
        self.last_update = chrono::Utc::now().timestamp_millis() as u64;
    }
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity);
        Self {
            sender,
            subscriber_count: 0,
        }
    }
    
    /// 获取发送器的克隆
    pub fn get_sender(&self) -> tokio::sync::broadcast::Sender<MonitorEvent> {
        self.sender.clone()
    }
}

#[async_trait::async_trait]
impl EventPublisher for EventBus {
    async fn publish(&self, event: MonitorEvent) -> Result<()> {
        self.sender.send(event)?;
        Ok(())
    }
    
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<MonitorEvent> {
        self.sender.subscribe()
    }
    
    fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new(max_data_points: usize) -> Self {
        Self {
            metrics: HashMap::new(),
            max_data_points,
        }
    }
    
    /// 记录指标
    pub fn record(&mut self, name: &str, value: f64) {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        let data_points = self.metrics.entry(name.to_string()).or_insert_with(Vec::new);
        data_points.push((timestamp, value));
        
        // 保持数据点数量在限制内
        if data_points.len() > self.max_data_points {
            data_points.remove(0);
        }
    }
    
    /// 获取指标的最新值
    pub fn get_latest(&self, name: &str) -> Option<f64> {
        self.metrics.get(name)?.last().map(|(_, value)| *value)
    }
    
    /// 获取指标的平均值
    pub fn get_average(&self, name: &str, last_n: Option<usize>) -> Option<f64> {
        let data_points = self.metrics.get(name)?;
        
        if data_points.is_empty() {
            return None;
        }
        
        let start_idx = if let Some(n) = last_n {
            if data_points.len() > n {
                data_points.len() - n
            } else {
                0
            }
        } else {
            0
        };
        
        let sum: f64 = data_points[start_idx..]
            .iter()
            .map(|(_, value)| value)
            .sum();
        
        let count = data_points.len() - start_idx;
        Some(sum / count as f64)
    }
    
    /// 清理过期数据
    pub fn cleanup_old_data(&mut self, max_age_ms: u64) {
        let cutoff_time = chrono::Utc::now().timestamp_millis() as u64 - max_age_ms;
        
        for data_points in self.metrics.values_mut() {
            data_points.retain(|(timestamp, _)| *timestamp > cutoff_time);
        }
    }
    
    /// 获取所有指标名称
    pub fn get_metric_names(&self) -> Vec<String> {
        self.metrics.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitor_stats() {
        let mut stats = MonitorStats::default();
        
        // 测试成功的价格获取
        stats.update_price_fetch(true, 100.0);
        stats.update_price_fetch(true, 200.0);
        stats.update_price_fetch(false, 1000.0);
        
        assert_eq!(stats.successful_trades, 2);
        assert_eq!(stats.failed_trades, 1);
        assert!((stats.success_rate() - 0.6667).abs() < 0.001);
        assert!((stats.average_response_time_ms - 433.33).abs() < 0.1);
    }
    
    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new(100);
        
        collector.record("response_time", 50.0);
        collector.record("response_time", 75.0);
        collector.record("response_time", 60.0);
        
        assert_eq!(collector.get_latest("response_time"), Some(60.0));
        
        let avg = collector.get_average("response_time", Some(3)).unwrap();
        assert!((avg - 61.67).abs() < 0.1);
        
        assert_eq!(collector.get_latest("non_existent"), None);
    }
    
    #[tokio::test]
    async fn test_event_bus() {
        let event_bus = EventBus::new(100);
        let mut receiver = event_bus.subscribe();
        
        let event = MonitorEvent::PriceUpdate {
            dex_name: "test".to_string(),
            symbol: "SOL/USDC".to_string(),
            price: 100.0,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        };
        
        event_bus.publish(event.clone()).await.unwrap();
        
        let received_event = receiver.recv().await.unwrap();
        
        match (event, received_event) {
            (MonitorEvent::PriceUpdate { dex_name: dex1, .. }, MonitorEvent::PriceUpdate { dex_name: dex2, .. }) => {
                assert_eq!(dex1, dex2);
            }
            _ => panic!("Event types don't match"),
        }
    }
}
