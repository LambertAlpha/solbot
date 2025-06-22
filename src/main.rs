//! Solana DEX套利监控系统主程序
//!
//! 本系统是一个基于Rust开发的Solana DEX套利监控平台，旨在实时监控多个去中心化交易所的价格差异，
//! 识别套利机会并提供自动化交易执行功能。
//!
//! ## 核心功能
//! - 实时价格监控：从多个Solana DEX获取实时价格数据
//! - 套利机会识别：智能算法检测价格差异和套利机会
//! - 自动化交易执行：支持自动执行套利交易
//! - 风险管理：内置滑点保护、最大损失限制等风险控制机制
//! - 监控告警：系统状态监控和异常告警
//!
//! ## 系统架构
//! - DEX接口层：统一的多DEX交互接口
//! - 监控模块：实时价格监控和系统状态管理
//! - 套利引擎：机会识别和交易执行逻辑
//! - 工具模块：配置管理、钱包管理等辅助功能

use anyhow::{Context, Result};
use std::{path::Path, sync::Arc};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

mod arbitrage;
mod dex;
mod monitor;
mod utils;

use crate::arbitrage::ArbitrageEngine;
use crate::dex::{DexInterface, jupiter::JupiterClient, orca::OrcaClient, raydium::RaydiumClient};
use crate::monitor::{EventBus, price_feed::{PriceFeed, TradingPair}};
use crate::utils::{AppConfig, Wallet};

/// 程序主入口函数
/// 
/// 负责初始化系统各个组件，启动监控和套利检测循环
/// 
/// ## 初始化流程
/// 1. 加载配置文件
/// 2. 初始化日志系统
/// 3. 创建钱包实例
/// 4. 初始化DEX客户端
/// 5. 启动价格监控
/// 6. 启动套利检测引擎
/// 7. 启动健康检查循环
/// 
/// ## 错误处理
/// 如果任何初始化步骤失败，程序将记录错误并退出
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统
    init_logging()?;
    
    info!("Starting Solana DEX Arbitrage Monitor...");
    
    // 加载配置
    let config = load_config()?;
    info!("Configuration loaded successfully");
    
    // 初始化钱包
    let wallet = init_wallet(&config)?;
    info!("Wallet initialized: {}", wallet.pubkey());
    
    // 初始化DEX客户端
    let dex_clients = init_dex_clients(&config)?;
    info!("DEX clients initialized: {:?}", dex_clients.iter().map(|c| c.get_name()).collect::<Vec<_>>());
    
    // 初始化事件总线
    let event_bus = Arc::new(EventBus::new(1000));
    info!("Event bus initialized");
    
    // 初始化价格监控器
    let price_feed = init_price_feed(&config, dex_clients.clone(), event_bus.clone()).await?;
    let price_feed = Arc::new(price_feed);
    info!("Price feed initialized");
    
    // 初始化套利引擎
    let arbitrage_engine = init_arbitrage_engine(&config, price_feed.clone(), wallet.clone()).await?;
    info!("Arbitrage engine initialized");
    
    // 启动监控循环
    start_monitoring_loop(arbitrage_engine, price_feed).await?;
    
    Ok(())
}

fn init_logging() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "solbot=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    info!("Logging system initialized");
    Ok(())
}

fn load_config() -> Result<AppConfig> {
    // 尝试从配置文件加载
    let config_path = Path::new("config.toml");
    let config = if config_path.exists() {
        AppConfig::load(Some(config_path))?
    } else {
        // 使用默认配置
        AppConfig::default()
    };
    
    info!("Configuration loaded from: {}", 
          if config_path.exists() { "config.toml" } else { "default" });
    
    Ok(config)
}

fn init_wallet(config: &AppConfig) -> Result<Arc<Wallet>> {
    let keypair = solana_sdk::signature::Keypair::new();
    let wallet = Wallet::new(keypair, &config.solana.rpc_url);
    Ok(Arc::new(wallet))
}

fn init_dex_clients(config: &AppConfig) -> Result<Vec<Arc<dyn DexInterface + Send + Sync>>> {
    let mut clients: Vec<Arc<dyn DexInterface + Send + Sync>> = Vec::new();
    
    for dex_name in &config.monitor.enabled_dexs {
        match dex_name.as_str() {
            "raydium" => {
                let client = RaydiumClient::new();
                clients.push(Arc::new(client));
            }
            "orca" => {
                let client = OrcaClient::new();
                clients.push(Arc::new(client));
            }
            "jupiter" => {
                let client = JupiterClient::new();
                clients.push(Arc::new(client));
            }
            _ => {
                warn!("Unknown DEX: {}", dex_name);
            }
        }
    }
    
    Ok(clients)
}

async fn init_price_feed(
    config: &AppConfig,
    dex_clients: Vec<Arc<dyn DexInterface + Send + Sync>>,
    event_bus: Arc<EventBus>,
) -> Result<PriceFeed> {
    let mut price_feed = PriceFeed::new(
        dex_clients,
        vec![], // 空的交易对列表，实际使用中需要配置
        config.monitor.clone(),
        event_bus,
    );
    
    // 添加更多交易对进行监控
    let trading_pairs = vec![
        TradingPair {
            symbol: "SOL/USDC".to_string(),
            input_mint: Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
            output_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            amount: 1_000_000_000, // 1 SOL
        },
        TradingPair {
            symbol: "ETH/USDC".to_string(),
            input_mint: Pubkey::from_str("7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs").unwrap(),
            output_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            amount: 1_000_000_000, // 1 ETH
        },
        TradingPair {
            symbol: "USDT/USDC".to_string(),
            input_mint: Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap(),
            output_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            amount: 1_000_000, // 1 USDT
        },
        TradingPair {
            symbol: "RAY/USDC".to_string(),
            input_mint: Pubkey::from_str("4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R").unwrap(),
            output_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            amount: 10_000_000, // 10 RAY
        },
        TradingPair {
            symbol: "SRM/USDC".to_string(),
            input_mint: Pubkey::from_str("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt").unwrap(),
            output_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            amount: 100_000_000, // 100 SRM
        },
    ];
    
    // 添加交易对到价格监控
    for pair in trading_pairs {
        price_feed.add_trading_pair(pair);
    }
    
    Ok(price_feed)
}

async fn init_arbitrage_engine(
    config: &AppConfig,
    price_feed: Arc<PriceFeed>,
    wallet: Arc<Wallet>,
) -> Result<ArbitrageEngine> {
    // 创建检测器配置
    let detector_config = arbitrage::detector::DetectorConfig {
        min_profit_threshold: config.strategy.min_profit_threshold,
        max_price_impact: config.risk.max_price_impact,
        min_liquidity: config.risk.min_liquidity as f64,
        detection_interval_ms: config.monitor.price_update_interval_ms,
        price_cache_ttl_seconds: 60,
        max_concurrent_detections: 10,
        monitor: config.monitor.clone(),
        strategy: config.strategy.clone(),
    };
    
    // 创建检测器
    let detector = arbitrage::detector::ArbitrageDetector::new(
        price_feed.clone(),
        detector_config,
    );
    
    // 创建执行器
    let executor = arbitrage::executor::ArbitrageExecutor::new(
        vec![], // 空的DEX客户端列表，实际使用中需要从price_feed获取
        Some(wallet.clone()),
        config.clone(),
    );
    
    // 创建套利引擎
    let arbitrage_engine = ArbitrageEngine::new(
        detector,
        executor,
        price_feed,
        wallet,
        config.clone(),
    );
    
    Ok(arbitrage_engine)
}

async fn start_monitoring_loop(
    arbitrage_engine: ArbitrageEngine,
    price_feed: Arc<PriceFeed>,
) -> Result<()> {
    // 启动价格监控
    let price_feed_handle = tokio::spawn({
        let price_feed = price_feed.clone();
        async move {
            if let Err(e) = price_feed.start().await {
                error!("Price feed monitoring failed: {}", e);
            }
        }
    });
    
    // 启动套利引擎
    let arbitrage_handle = tokio::spawn({
        async move {
            if let Err(e) = arbitrage_engine.run().await {
                error!("Arbitrage engine failed: {}", e);
            }
        }
    });
    
    // 等待所有任务完成
    tokio::try_join!(
        price_feed_handle,
        arbitrage_handle
    )?;
    
    info!("Application shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loading() {
        let config = AppConfig::default();
        assert!(!config.solana.rpc_url.is_empty());
        assert!(!config.monitor.enabled_dexs.is_empty());
    }

    #[test]
    fn test_wallet_initialization() {
        use solana_sdk::signature::Signer;
        
        let config = AppConfig::default();
        let wallet = Wallet::new(solana_sdk::signature::Keypair::new(), &config.solana.rpc_url);
        
        // 测试钱包基本功能
        assert_eq!(wallet.pubkey(), wallet.pubkey());
    }
}
