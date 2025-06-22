# Solana DEX套利监控系统

[![Rust](https://img.shields.io/badge/Rust-1.70+-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/Build-Passing-brightgreen.svg)]()

一个基于Rust开发的高性能Solana DEX套利监控系统，支持实时价格监控、智能套利机会识别和自动化交易执行。

## 🚀 核心特性

### 实时监控
- **多DEX支持**: 集成Raydium、Orca、Jupiter等主流Solana DEX
- **毫秒级响应**: 500ms价格更新频率，快速捕捉套利机会
- **智能缓存**: LRU缓存机制，减少API调用，提高性能
- **健康检查**: 实时监控DEX连接状态，自动故障转移

### 套利识别
- **多策略支持**: 简单套利、三角套利、统计套利等
- **智能评分**: 基于利润率、流动性、风险的综合评分系统
- **实时检测**: 持续监控价格差异，自动识别套利机会
- **风险控制**: 内置滑点保护、流动性检查等风险控制机制

### 交易执行
- **原子性执行**: 确保交易要么完全成功，要么完全失败
- **智能重试**: 自动重试失败的交易，提高成功率
- **费用优化**: 最小化交易费用，最大化收益
- **MEV保护**: 防止MEV攻击，保护交易安全

### 系统监控
- **实时统计**: 详细的执行统计和性能指标
- **事件通知**: 支持邮件、Slack等多种通知方式
- **日志记录**: 完整的操作日志，便于问题排查
- **性能分析**: 交易历史分析和收益统计

## 📋 系统要求

### 硬件要求
- **CPU**: 4核心以上
- **内存**: 8GB以上
- **存储**: 100GB SSD
- **网络**: 稳定的互联网连接，延迟<50ms

### 软件要求
- **Rust**: 1.70+
- **Solana CLI**: 1.16+
- **操作系统**: Linux/macOS/Windows
- **数据库**: PostgreSQL 13+ (可选)
- **缓存**: Redis 6+ (可选)

## 🛠️ 安装指南

### 1. 克隆项目
```bash
git clone https://github.com/your-repo/solana-arbitrage-monitor.git
cd solana-arbitrage-monitor
```

### 2. 安装依赖
```bash
# 安装Rust (如果未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/v1.16.0/install)"

# 编译项目
cargo build --release
```

### 3. 配置环境
```bash
# 复制配置文件
cp config.toml.example config.toml

# 编辑配置文件
nano config.toml
```

### 4. 生成钱包
```bash
# 创建钱包目录
mkdir -p wallet

# 生成新钱包 (生产环境请使用硬件钱包)
solana-keygen new --outfile ./wallet/keypair.json

# 或者导入现有钱包
solana-keygen recover 'prompt:?key=0/0' -o ./wallet/keypair.json
```

### 5. 运行系统
```bash
# 开发模式
cargo run

# 生产模式
cargo run --release
```

## ⚙️ 配置说明

### 基础配置
```toml
[solana]
rpc_url = "https://api.mainnet-beta.solana.com"
ws_url = "wss://api.mainnet-beta.solana.com"
commitment = "confirmed"
network = "mainnet-beta"

[wallet]
keypair_path = "./wallet/keypair.json"
max_balance_usage = 0.8
use_hardware_wallet = false
```

### 监控配置
```toml
[monitor]
price_update_interval_ms = 500
max_price_staleness_ms = 5000
enabled_dexs = ["raydium", "orca", "jupiter", "serum"]
trading_pairs = ["SOL/USDC", "ETH/USDC", "USDT/USDC"]
```

### 策略配置
```toml
[strategy]
min_profit_threshold = 0.1
max_trade_amount = 1000000000
slippage_tolerance = 0.2
auto_execute = true
execution_delay_ms = 100
```

### 风险控制
```toml
[risk]
max_position_size = 10000000000
max_daily_loss = 1000000000
max_slippage = 1.0
min_liquidity = 100000000000
max_price_impact = 2.0
cooldown_period_ms = 5000
```

## 📊 使用指南

### 启动监控
```bash
# 启动价格监控
cargo run --bin solbot

# 查看日志
tail -f logs/arbitrage.log
```

### 查看状态
```bash
# 查看系统状态
curl http://localhost:8080/api/v1/status

# 查看价格数据
curl http://localhost:8080/api/v1/prices?symbol=SOL/USDC

# 查看套利机会
curl http://localhost:8080/api/v1/opportunities?min_profit=0.5
```

### 监控指标
- **价格更新延迟**: 平均<100ms
- **API响应时间**: 平均<50ms
- **交易执行时间**: 平均<2秒
- **系统可用性**: >99.9%
- **数据准确性**: >99.99%

## 🔧 开发指南

### 项目结构
```
solbot/
├── src/
│   ├── main.rs              # 程序入口
│   ├── dex/                 # DEX接口层
│   │   ├── jupiter.rs       # Jupiter API封装
│   │   ├── raydium.rs       # Raydium SDK封装
│   │   ├── orca.rs          # Orca SDK封装
│   │   └── mod.rs
│   ├── arbitrage/           # 套利逻辑
│   │   ├── detector.rs      # 价差检测
│   │   ├── executor.rs      # 交易执行
│   │   └── mod.rs
│   ├── monitor/             # 监控模块
│   │   ├── price_feed.rs    # 实时价格
│   │   └── mod.rs
│   └── utils/               # 工具函数
│       ├── wallet.rs        # 钱包管理
│       └── config.rs        # 配置管理
├── config.toml              # 配置文件
├── Cargo.toml               # 依赖配置
└── README.md               # 项目文档
```

### 添加新的DEX支持
```rust
// 实现DexInterface trait
#[async_trait::async_trait]
impl DexInterface for NewDexClient {
    async fn get_price(&self, input_mint: &Pubkey, output_mint: &Pubkey, amount: u64) -> Result<Price> {
        // 实现价格获取逻辑
    }
    
    async fn execute_trade(&self, trade_info: &TradeInfo) -> Result<String> {
        // 实现交易执行逻辑
    }
    
    fn get_name(&self) -> &str {
        "new_dex"
    }
    
    async fn health_check(&self) -> Result<bool> {
        // 实现健康检查逻辑
    }
}
```

### 自定义套利策略
```rust
// 实现ArbitrageStrategy trait
pub struct CustomStrategy {
    // 策略特定参数
}

#[async_trait::async_trait]
impl ArbitrageStrategy for CustomStrategy {
    async fn find_opportunities(&self) -> Result<Vec<ArbitrageOpportunity>> {
        // 实现机会识别逻辑
    }
    
    async fn should_execute(&self, opportunity: &ArbitrageOpportunity) -> Result<bool> {
        // 实现执行决策逻辑
    }
}
```

## 🧪 测试

### 运行测试
```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_orca_price_calculation

# 运行集成测试
cargo test --test integration_tests
```

### 测试覆盖率
```bash
# 安装tarpaulin
cargo install cargo-tarpaulin

# 生成覆盖率报告
cargo tarpaulin --out Html
```

## 📈 性能优化

### 并发优化
- 使用tokio异步运行时
- 多线程并发处理
- 连接池复用
- 批量操作优化

### 内存优化
- LRU缓存策略
- 对象池复用
- 内存映射文件
- 垃圾回收优化

### 网络优化
- 连接复用
- 请求压缩
- 超时控制
- 重试机制

## 🔒 安全考虑

### 私钥安全
- 使用硬件钱包
- 私钥加密存储
- 访问权限控制
- 定期密钥轮换

### 网络安全
- HTTPS/WSS加密
- API速率限制
- 防火墙保护
- VPN连接

### 交易安全
- 交易验证
- 滑点保护
- 余额检查
- 风险限制

## 🚨 风险警告

**⚠️ 重要提醒**

- 加密货币交易存在重大风险，可能导致资金损失
- 本系统仅供学习和研究目的
- 在使用本系统进行实际交易前，请充分了解相关风险
- 建议先在测试网进行充分测试
- 开发者不对使用本系统造成的任何损失承担责任

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🤝 贡献指南

欢迎贡献代码！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解贡献指南。

### 贡献方式
- 🐛 报告Bug
- 💡 提出新功能
- 📝 改进文档
- 🔧 提交代码

## 📞 支持

如果您遇到问题或有疑问，请：

1. 查看 [文档](docs/)
2. 搜索 [Issues](../../issues)
3. 创建新的 [Issue](../../issues/new)
4. 联系开发团队

## 🙏 致谢

感谢以下开源项目的支持：

- [Solana](https://solana.com/) - 高性能区块链平台
- [Tokio](https://tokio.rs/) - 异步运行时
- [Serde](https://serde.rs/) - 序列化框架
- [Tracing](https://tracing.rs/) - 日志和追踪

---

**免责声明**: 本系统仅供学习和研究目的。加密货币交易存在重大风险，可能导致资金损失。在使用本系统进行实际交易前，请充分了解相关风险并谨慎操作。 