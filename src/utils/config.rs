use anyhow::Result;
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub commitment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub keypair_path: String,
    pub max_balance_usage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub price_update_interval_ms: u64,
    pub max_price_staleness_ms: u64,
    pub enabled_dexs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub min_profit_threshold: f64,
    pub max_trade_amount: u64,
    pub slippage_tolerance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_size: u64,
    pub max_daily_loss: f64,
    pub max_slippage: f64,
    pub min_liquidity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub solana: SolanaConfig,
    pub wallet: WalletConfig,
    pub monitor: MonitorConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub logging: LoggingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            solana: SolanaConfig {
                rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
                ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
                commitment: "confirmed".to_string(),
            },
            wallet: WalletConfig {
                keypair_path: "./wallet/keypair.json".to_string(),
                max_balance_usage: 0.8,
            },
            monitor: MonitorConfig {
                price_update_interval_ms: 1000,
                max_price_staleness_ms: 10000,
                enabled_dexs: vec!["raydium".to_string(), "orca".to_string(), "jupiter".to_string()],
            },
            strategy: StrategyConfig {
                min_profit_threshold: 0.5,
                max_trade_amount: 1_000_000_000, // 1 SOL
                slippage_tolerance: 0.3,
            },
            risk: RiskConfig {
                max_position_size: 10_000_000_000, // 10 SOL
                max_daily_loss: 1_000_000_000.0,     // 1 SOL
                max_slippage: 1.0,
                min_liquidity: 100_000_000_000,    // 100 SOL
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: Some("./logs/arbitrage.log".to_string()),
            },
        }
    }
}

impl AppConfig {
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let mut config = Config::default();

        // 加载默认配置
        config.merge(File::from_str(
            &toml::to_string(&AppConfig::default())?,
            config::FileFormat::Toml,
        ))?;

        // 加载配置文件（如果存在）
        if let Some(path) = config_path {
            if path.exists() {
                config.merge(File::from(path))?;
            }
        }

        // 加载环境变量（前缀：SOLBOT_）
        config.merge(Environment::with_prefix("SOLBOT").separator("_"))?;

        // 手动解析配置
        let app_config = AppConfig {
            solana: SolanaConfig {
                rpc_url: config.get_string("solana.rpc_url")?,
                ws_url: config.get_string("solana.ws_url")?,
                commitment: config.get_string("solana.commitment")?,
            },
            wallet: WalletConfig {
                keypair_path: config.get_string("wallet.keypair_path")?,
                max_balance_usage: config.get_float("wallet.max_balance_usage")?,
            },
            monitor: MonitorConfig {
                price_update_interval_ms: config.get_int("monitor.price_update_interval_ms")? as u64,
                max_price_staleness_ms: config.get_int("monitor.max_price_staleness_ms")? as u64,
                enabled_dexs: config.get_array("monitor.enabled_dexs")?
                    .into_iter()
                    .map(|v| v.into_string())
                    .collect::<Result<Vec<_>, _>>()?,
            },
            strategy: StrategyConfig {
                min_profit_threshold: config.get_float("strategy.min_profit_threshold")?,
                max_trade_amount: config.get_int("strategy.max_trade_amount")? as u64,
                slippage_tolerance: config.get_float("strategy.slippage_tolerance")?,
            },
            risk: RiskConfig {
                max_position_size: config.get_int("risk.max_position_size")? as u64,
                max_daily_loss: config.get_float("risk.max_daily_loss")?,
                max_slippage: config.get_float("risk.max_slippage")?,
                min_liquidity: config.get_int("risk.min_liquidity")? as u64,
            },
            logging: LoggingConfig {
                level: config.get_string("logging.level")?,
                file: config.get_string("logging.file").ok(),
            },
        };

        Ok(app_config)
    }

    pub fn validate(&self) -> Result<()> {
        // 验证Solana配置
        if self.solana.rpc_url.is_empty() {
            anyhow::bail!("RPC URL cannot be empty");
        }

        // 验证钱包配置
        if self.wallet.max_balance_usage <= 0.0 || self.wallet.max_balance_usage > 1.0 {
            anyhow::bail!("Max balance usage must be between 0 and 1");
        }

        // 验证监控配置
        if self.monitor.price_update_interval_ms == 0 {
            anyhow::bail!("Price update interval must be greater than 0");
        }

        // 验证策略配置
        if self.strategy.min_profit_threshold < 0.0 {
            anyhow::bail!("Min profit threshold cannot be negative");
        }

        // 验证风险配置
        if self.risk.max_daily_loss <= 0.0 {
            anyhow::bail!("Max daily loss must be greater than 0");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(!config.solana.rpc_url.is_empty());
        assert_eq!(config.wallet.max_balance_usage, 0.8);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        assert!(config.validate().is_ok());

        // 测试无效配置
        config.wallet.max_balance_usage = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_load_config_from_file() {
        // 创建临时TOML文件
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
            [solana]
            rpc_url = "https://test-rpc.solana.com"
            ws_url = "wss://test-rpc.solana.com"
            commitment = "confirmed"
            
            [wallet]
            keypair_path = "./test-wallet.json"
            max_balance_usage = 0.5
            
            [monitor]
            price_update_interval_ms = 2000
            max_price_staleness_ms = 15000
            enabled_dexs = ["test_dex"]
            
            [strategy]
            min_profit_threshold = 1.0
            max_trade_amount = 500000000
            slippage_tolerance = 0.5
            
            [risk]
            max_position_size = 5000000000
            max_daily_loss = 500000000
            max_slippage = 2.0
            min_liquidity = 50000000000
            
            [logging]
            level = "debug"
            file = "./test-logs/arbitrage.log"
        "#;
        
        // 写入配置内容
        temp_file.write_all(config_content.as_bytes()).unwrap();
        
        // 获取文件路径并添加.toml扩展名
        let temp_path = temp_file.path();
        let config_path = temp_path.with_extension("toml");
        std::fs::rename(temp_path, &config_path).unwrap();
        
        // 测试加载配置
        let config = AppConfig::load(Some(&config_path)).unwrap();
        
        // 验证配置值
        assert_eq!(config.solana.rpc_url, "https://test-rpc.solana.com");
        assert_eq!(config.wallet.max_balance_usage, 0.5);
        assert_eq!(config.monitor.price_update_interval_ms, 2000);
        assert_eq!(config.strategy.min_profit_threshold, 1.0);
        assert_eq!(config.risk.max_daily_loss, 500000000.0);
        assert_eq!(config.logging.level, "debug");
        
        // 清理临时文件
        std::fs::remove_file(config_path).unwrap();
    }
}
