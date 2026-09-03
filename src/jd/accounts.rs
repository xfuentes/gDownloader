use std::sync::Arc;

use anyhow::Result;
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::jd::JdApi;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountQuery {
    pub user_name: bool,
    pub enabled: bool,
    pub valid: bool,
    pub valid_until: bool,
    pub traffic_left: bool,
    pub traffic_max: bool,
    pub error: bool,
    pub start_at: i32,
    pub max_results: i32,
    #[serde(rename = "UUIDList")]
    pub uuid_list: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct AccountStorable {
    pub uuid: u64,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub valid: bool,
    pub hostname: String,
    pub username: Option<String>,
    pub error_type: Option<String>,
    pub error_string: Option<String>,
    pub valid_until: Option<i64>,
    pub traffic_left: Option<i64>,
    pub traffic_max: Option<i64>,
}

/// Accessor for JDownloader's `accountsV2` RemoteAPI.
#[derive(Clone)]
pub struct JdAccounts {
    api: Arc<JdApi>,
}

impl JdAccounts {
    pub fn new(api: Arc<JdApi>) -> Self {
        Self { api }
    }

    pub fn api(&self) -> &Arc<JdApi> {
        &self.api
    }

    pub fn is_ready(&self) -> bool {
        self.api.is_ready()
    }

    pub fn list_premium_hoster(&self) -> Result<Vec<String>> {
        let value = self.api.call("accountsV2/listPremiumHoster", &[])?;
        serde_json::from_value(value).map_err(|e| anyhow::anyhow!("Failed to parse hosters: {}", e))
    }

    pub fn list_accounts(&self, query: &AccountQuery) -> Result<Vec<AccountStorable>> {
        let query_json = serde_json::to_string(query)?;
        let p: &str = &query_json;
        let value = self.api.call("accountsV2/listAccounts", &[p])?;
        let result: Result<Vec<AccountStorable>, _> = serde_json::from_value(value);
        match result {
            Ok(list) => {
                info!("list_accounts parsed {} accounts", list.len());
                Ok(list)
            }
            Err(e) => {
                error!("Failed to parse accounts: {}", e);
                Err(anyhow::anyhow!("Failed to parse accounts: {}", e))
            }
        }
    }

    pub fn add_account(&self, hoster: &str, username: &str, password: &str) -> Result<bool> {
        let value = self.api.call(
            "accountsV2/addAccount",
            &[hoster, username, password],
        )?;
        match value {
            serde_json::Value::Bool(b) => Ok(b),
            serde_json::Value::Null => Ok(true),
            serde_json::Value::String(s) if s.is_empty() => Ok(true),
            _ => Ok(false),
        }
    }

    pub fn remove_accounts(&self, ids: &[u64]) -> Result<()> {
        info!("remove_accounts ids: {:?}", ids);
        let ids_json = serde_json::to_string(ids)?;
        let p: &str = &ids_json;
        self.api.call("accountsV2/removeAccounts", &[p])?;
        info!("remove_accounts OK");
        Ok(())
    }

    #[allow(dead_code)]
    pub fn enable_accounts(&self, ids: &[u64]) -> Result<()> {
        let ids_json = serde_json::to_string(ids)?;
        let p: &str = &ids_json;
        self.api.call("accountsV2/enableAccounts", &[p])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn disable_accounts(&self, ids: &[u64]) -> Result<()> {
        let ids_json = serde_json::to_string(ids)?;
        let p: &str = &ids_json;
        self.api.call("accountsV2/disableAccounts", &[p])?;
        Ok(())
    }

    pub fn refresh_accounts(&self, ids: &[u64], force: bool) -> Result<()> {
        let ids_json = serde_json::to_string(ids)?;
        let p1: &str = &ids_json;
        let p2: &str = if force { "true" } else { "false" };
        self.api
            .call("accountsV2/refreshAccounts", &[p1, p2])?;
        Ok(())
    }
}
