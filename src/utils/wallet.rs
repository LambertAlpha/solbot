use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::{
    fs,
    path::Path,
    sync::Arc,
};
use tracing::{error, info};

pub struct Wallet {
    keypair: Keypair,
    rpc_client: Arc<RpcClient>,
}

impl Wallet {
    pub fn new(keypair: Keypair, rpc_url: &str) -> Self {
        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        ));
        Self {
            keypair,
            rpc_client,
        }
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let keypair_data = fs::read(path)
            .with_context(|| format!("Failed to read keypair file: {}", path.display()))?;
        
        let keypair = if keypair_data.len() == 64 {
            // 原始字节格式
            Keypair::from_bytes(&keypair_data)
                .with_context(|| "Failed to parse keypair from bytes")?
        } else {
            // JSON格式
            let keypair_json: serde_json::Value = serde_json::from_slice(&keypair_data)
                .with_context(|| "Failed to parse keypair JSON")?;
            
            if let Some(secret_key) = keypair_json.get("secretKey") {
                if let Some(secret_key_array) = secret_key.as_array() {
                    let secret_key_bytes: Vec<u8> = secret_key_array
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                        .collect();
                    Keypair::from_bytes(&secret_key_bytes)
                        .with_context(|| "Failed to parse keypair from secret key array")?
                } else {
                    anyhow::bail!("Invalid secret key format in JSON")
                }
            } else {
                anyhow::bail!("No secret key found in JSON")
            }
        };

        info!("Wallet loaded successfully: {}", keypair.pubkey());
        Ok(Self {
            keypair,
            rpc_client: Arc::new(RpcClient::new("https://api.mainnet-beta.solana.com")),
        })
    }

    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }

    pub async fn get_balance(&self) -> Result<u64> {
        let balance = self.rpc_client
            .get_balance(&self.keypair.pubkey())
            .with_context(|| "Failed to get wallet balance")?;
        
        info!("Wallet balance: {} lamports", balance);
        Ok(balance)
    }

    pub async fn get_token_balance(&self, token_account: &Pubkey) -> Result<u64> {
        let balance = self.rpc_client
            .get_token_account_balance(token_account)
            .with_context(|| "Failed to get token balance")?;
        
        Ok(balance.ui_amount.unwrap_or(0.0) as u64)
    }

    pub async fn get_token_accounts(&self) -> Result<Vec<Pubkey>> {
        let accounts = self.rpc_client
            .get_token_accounts_by_owner(
                &self.keypair.pubkey(),
                TokenAccountsFilter::ProgramId(spl_token::id()),
            )
            .with_context(|| "Failed to get token accounts")?;
        
        let token_accounts: Vec<Pubkey> = accounts
            .iter()
            .map(|account| account.pubkey.parse().unwrap())
            .collect();
        
        Ok(token_accounts)
    }

    pub async fn sign_and_send_transaction(&self, transaction: Transaction) -> Result<String> {
        let signature = self.rpc_client
            .send_and_confirm_transaction(&transaction)
            .with_context(|| "Failed to send transaction")?;
        
        info!("Transaction sent successfully: {}", signature);
        Ok(signature.to_string())
    }

    pub async fn sign_transaction(&self, mut transaction: Transaction) -> Result<Transaction> {
        let blockhash = self.rpc_client.get_latest_blockhash()?;
        transaction.sign(&[&self.keypair], blockhash);
        Ok(transaction)
    }

    pub async fn get_recent_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        let blockhash = self.rpc_client
            .get_latest_blockhash()
            .with_context(|| "Failed to get recent blockhash")?;
        Ok(blockhash)
    }

    pub async fn confirm_transaction(&self, signature: &str) -> Result<bool> {
        let signature = signature.parse::<solana_sdk::signature::Signature>()
            .with_context(|| "Invalid signature format")?;
        
        let confirmation = self.rpc_client
            .confirm_transaction(&signature)
            .with_context(|| "Failed to confirm transaction")?;
        
        Ok(confirmation)
    }

    pub async fn get_transaction_status(&self, signature: &str) -> Result<Option<solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta>> {
        let signature = signature.parse::<solana_sdk::signature::Signature>()
            .with_context(|| "Invalid signature format")?;
        
        let status = self.rpc_client
            .get_transaction(&signature, solana_transaction_status::UiTransactionEncoding::Json)
            .with_context(|| "Failed to get transaction status")?;
        
        Ok(Some(status))
    }

    pub async fn estimate_fee(&self, transaction: &Transaction) -> Result<u64> {
        let fee = self.rpc_client
            .get_fee_for_message(&transaction.message)
            .with_context(|| "Failed to estimate transaction fee")?;
        
        Ok(fee)
    }

    pub async fn check_account_exists(&self, account: &Pubkey) -> Result<bool> {
        let account_info = self.rpc_client
            .get_account(account);
        
        Ok(account_info.is_ok())
    }

    pub async fn get_account_info(&self, account: &Pubkey) -> Result<Option<solana_sdk::account::Account>> {
        let account_info = self.rpc_client
            .get_account(account)
            .with_context(|| "Failed to get account info")?;
        
        Ok(Some(account_info))
    }

    pub fn rpc_client(&self) -> &Arc<RpcClient> {
        &self.rpc_client
    }

    pub async fn health_check(&self) -> Result<bool> {
        match self.get_balance().await {
            Ok(_) => {
                info!("Wallet health check passed");
                Ok(true)
            }
            Err(e) => {
                error!("Wallet health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

impl Clone for Wallet {
    fn clone(&self) -> Self {
        Self {
            keypair: Keypair::from_bytes(&self.keypair.to_bytes()).unwrap(),
            rpc_client: self.rpc_client.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::fs;

    #[tokio::test]
    async fn test_wallet_creation() {
        let keypair = Keypair::new();
        let wallet = Wallet::new(keypair, "https://api.mainnet-beta.solana.com");
        assert_eq!(wallet.pubkey(), wallet.keypair().pubkey());
    }

    #[test]
    fn test_load_wallet_from_file() {
        let keypair = Keypair::new();
        let temp_file = NamedTempFile::new().unwrap();
        
        // 保存为JSON格式
        let keypair_json = serde_json::json!({
            "secretKey": keypair.to_bytes().to_vec()
        });
        fs::write(&temp_file, serde_json::to_string(&keypair_json).unwrap()).unwrap();
        
        let loaded_wallet = Wallet::load_from_file(temp_file.path()).unwrap();
        assert_eq!(loaded_wallet.pubkey(), keypair.pubkey());
    }

    #[test]
    fn test_load_wallet_from_bytes() {
        let keypair = Keypair::new();
        let temp_file = NamedTempFile::new().unwrap();
        
        // 保存为原始字节格式
        fs::write(&temp_file, keypair.to_bytes()).unwrap();
        
        let loaded_wallet = Wallet::load_from_file(temp_file.path()).unwrap();
        assert_eq!(loaded_wallet.pubkey(), keypair.pubkey());
    }
}
