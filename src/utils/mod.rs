//! 工具模块
//!
//! 本模块提供系统运行所需的各种工具和辅助功能。
//! 包括配置管理、钱包管理、日志记录等核心工具。
//!
//! ## 核心组件
//! - **Config**: 配置管理，支持文件、环境变量等多种配置源
//! - **Wallet**: 钱包管理，处理Solana钱包的创建、签名和交易
//! - **Logger**: 日志系统，提供结构化的日志记录功能
//! - **Error**: 错误处理，统一的错误类型和错误处理机制
//!
//! ## 主要功能
//! - 配置文件加载和验证
//! - 钱包密钥管理和安全存储
//! - 交易签名和广播
//! - 系统日志记录和监控
//! - 错误处理和恢复机制
//!
//! ## 安全特性
//! - 私钥加密存储
//! - 安全的密钥生成
//! - 交易签名验证
//! - 访问权限控制

pub mod config;
pub mod wallet;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::sync::Arc;
use std::path::Path;

/// 应用程序配置
/// 
/// 定义系统运行所需的所有配置参数
/// 
/// ## 配置来源优先级
/// 1. 环境变量 (最高优先级)
/// 2. 配置文件
/// 3. 默认值 (最低优先级)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Solana网络配置
    pub solana: SolanaConfig,
    /// 钱包配置
    pub wallet: WalletConfig,
    /// 监控配置
    pub monitor: MonitorConfig,
    /// 套利策略配置
    pub strategy: StrategyConfig,
    /// 风险控制配置
    pub risk: RiskConfig,
    /// 日志配置
    pub logging: LoggingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            solana: SolanaConfig {
                rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
                ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                network: "mainnet-beta".to_string(),
            },
            wallet: WalletConfig {
                keypair_path: "./wallet/keypair.json".to_string(),
                max_balance_usage: 0.8,
                use_hardware_wallet: false,
                encryption_password: None,
            },
            monitor: MonitorConfig {
                price_update_interval_ms: 1000,
                max_price_staleness_ms: 10000,
                enabled_dexs: vec!["raydium".to_string(), "orca".to_string(), "jupiter".to_string()],
                trading_pairs: vec!["SOL/USDC".to_string()],
            },
            strategy: StrategyConfig {
                min_profit_threshold: 0.5,
                max_trade_amount: 1000000000,
                slippage_tolerance: 0.3,
                auto_execute: true,
                execution_delay_ms: 100,
            },
            risk: RiskConfig {
                max_position_size: 10000000000,
                max_daily_loss: 1000000000.0,
                max_slippage: 1.0,
                min_liquidity: 100000000000,
                max_price_impact: 2.0,
                cooldown_period_ms: 5000,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: "./logs/arbitrage.log".to_string(),
                console_output: true,
                max_file_size_mb: 100,
                max_files: 10,
            },
        }
    }
}

impl AppConfig {
    /// 从配置文件加载配置
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        // 这里可以添加配置文件加载逻辑
        Ok(Self::default())
    }
}

/// Solana网络配置
/// 
/// 配置Solana网络的连接参数和网络选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    /// RPC节点URL
    pub rpc_url: String,
    /// WebSocket节点URL
    pub ws_url: String,
    /// 交易确认级别
    pub commitment: String,
    /// 网络类型 (mainnet-beta, testnet, devnet)
    pub network: String,
}

/// 钱包配置
/// 
/// 配置钱包相关的参数和安全设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// 钱包密钥文件路径
    pub keypair_path: String,
    /// 最大余额使用比例 (0.0-1.0)
    pub max_balance_usage: f64,
    /// 是否启用硬件钱包
    pub use_hardware_wallet: bool,
    /// 钱包加密密码 (可选)
    pub encryption_password: Option<String>,
}

/// 监控配置
/// 
/// 配置价格监控和系统监控的参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// 价格更新间隔 (毫秒)
    pub price_update_interval_ms: u64,
    /// 最大价格过期时间 (毫秒)
    pub max_price_staleness_ms: u64,
    /// 启用的DEX列表
    pub enabled_dexs: Vec<String>,
    /// 监控的交易对列表
    pub trading_pairs: Vec<String>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            price_update_interval_ms: 1000,
            max_price_staleness_ms: 10000,
            enabled_dexs: vec!["raydium".to_string(), "orca".to_string(), "jupiter".to_string()],
            trading_pairs: vec!["SOL/USDC".to_string()],
        }
    }
}

/// 套利策略配置
/// 
/// 配置套利策略的参数和阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// 最小利润阈值 (百分比)
    pub min_profit_threshold: f64,
    /// 最大交易金额 (lamports)
    pub max_trade_amount: u64,
    /// 滑点容忍度 (百分比)
    pub slippage_tolerance: f64,
    /// 是否启用自动执行
    pub auto_execute: bool,
    /// 执行延迟 (毫秒)
    pub execution_delay_ms: u64,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            min_profit_threshold: 0.5,
            max_trade_amount: 1000000000,
            slippage_tolerance: 0.3,
            auto_execute: true,
            execution_delay_ms: 100,
        }
    }
}

/// 风险控制配置
/// 
/// 配置风险控制的各种参数和限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// 最大持仓规模 (lamports)
    pub max_position_size: u64,
    /// 最大日损失 (lamports)
    pub max_daily_loss: f64,
    /// 最大滑点 (百分比)
    pub max_slippage: f64,
    /// 最小流动性要求 (lamports)
    pub min_liquidity: u64,
    /// 最大价格影响 (百分比)
    pub max_price_impact: f64,
    /// 冷却期 (毫秒)
    pub cooldown_period_ms: u64,
}

/// 日志配置
/// 
/// 配置日志记录的各种参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// 日志级别 (debug, info, warn, error)
    pub level: String,
    /// 日志文件路径
    pub file: String,
    /// 是否输出到控制台
    pub console_output: bool,
    /// 日志轮转大小 (MB)
    pub max_file_size_mb: u64,
    /// 保留的日志文件数量
    pub max_files: u32,
}

/// 钱包管理结构
/// 
/// 提供Solana钱包的完整管理功能，包括密钥管理、交易签名等
/// 
/// ## 安全特性
/// - 私钥加密存储
/// - 安全的密钥生成
/// - 交易签名验证
/// - 余额监控和保护
pub struct Wallet {
    /// 钱包密钥对
    keypair: Keypair,
    /// RPC客户端
    rpc_client: Arc<solana_client::rpc_client::RpcClient>,
    /// 钱包地址
    pubkey: Pubkey,
    /// 最后余额更新时间
    last_balance_update: u64,
    /// 当前余额 (lamports)
    balance: u64,
}

impl Wallet {
    /// 创建新的钱包实例
    pub fn new(keypair: Keypair, rpc_url: &str) -> Self {
        let pubkey = keypair.pubkey();
        let rpc_client = Arc::new(solana_client::rpc_client::RpcClient::new(rpc_url.to_string()));
        
        Self {
            keypair,
            rpc_client,
            pubkey,
            last_balance_update: 0,
            balance: 0,
        }
    }
    
    /// 获取钱包公钥
    pub fn pubkey(&self) -> Pubkey {
        self.pubkey
    }
}

/// 交易结果
/// 
/// 记录交易执行的详细结果信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    /// 交易签名
    pub signature: String,
    /// 执行状态
    pub success: bool,
    /// 交易费用 (lamports)
    pub fee: u64,
    /// 确认时间 (毫秒)
    pub confirmation_time_ms: u64,
    /// 错误信息 (如果失败)
    pub error_message: Option<String>,
    /// 交易详情
    pub details: TransactionDetails,
}

/// 交易详情
/// 
/// 包含交易的详细信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetails {
    /// 输入账户
    pub input_accounts: Vec<Pubkey>,
    /// 输出账户
    pub output_accounts: Vec<Pubkey>,
    /// 交易金额
    pub amount: u64,
    /// 交易类型
    pub transaction_type: String,
    /// 时间戳
    pub timestamp: u64,
} 