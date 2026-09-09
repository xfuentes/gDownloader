// gDownloader
// Copyright (c) 2026. Xavier Fuentes <xfuentes-dev@serviam.cc>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

mod config;
mod downloads;
mod extraction;
mod linkgrabber;
mod system;

use std::time::Duration;

use anyhow::Result;
use log::debug;
use reqwest::blocking::Client;
use serde_json::Value;

#[derive(Clone)]
/// Basic client to communicate with JDownloader's local API.
///
/// The deprecated API is enabled on `INTERNAL_JD_PORT` by `JdProcess`. It is
/// unauthenticated and exposes the `/config/get`, `/config/set`, etc. endpoints
/// of the RemoteAPI.
pub struct JdApi {
    pub(super) client: Client,
    pub(super) base_url: String,
}

impl JdApi {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Calls a V2-style endpoint with a single JSON `query` parameter.
    pub(super) fn get_v2(&self, endpoint: &str, query_key: &str, params: Value) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let params_str = params.to_string();
        debug!(
            "JDownloader API v2: {}?{}={}",
            endpoint, query_key, params_str
        );
        let response = self
            .client
            .get(&url)
            .query(&[(query_key, &params_str)])
            .timeout(Duration::from_secs(10))
            .send()?;
        let status = response.status();
        let text = response.text()?;
        debug!(
            "JDownloader API v2 response ({} {}): {}",
            status, endpoint, text
        );
        if !status.is_success() {
            anyhow::bail!(
                "JDownloader API v2 {} returned {}: {}",
                endpoint,
                status,
                text
            );
        }
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Invalid JSON from JDownloader v2 ({}): {}", e, text))?;
        Ok(unwrap_value(value))
    }

    /// Calls an arbitrary RemoteAPI method with positional parameters.
    ///
    /// The request is of the form `/{method}?{param0}&{param1}&...`.
    /// Parameters are JSON-encoded by the caller when required.
    pub fn call(&self, method: &str, params: &[&str]) -> Result<Value> {
        let url = format!("{}/{}?", self.base_url, method);
        let mut request = self.client.get(&url);
        let query: Vec<(String, String)> = params
            .iter()
            .enumerate()
            .map(|(i, p)| (format!("p{}", i), p.to_string()))
            .collect();
        if !query.is_empty() {
            request = request.query(&query);
        }
        debug!(
            "JDownloader API call: {} with {} params: {:?}",
            method,
            params.len(),
            params
        );
        let response = request.timeout(Duration::from_secs(10)).send()?;
        let status = response.status();
        let text = response.text()?;
        debug!(
            "JDownloader API call response ({} {}): {}",
            status, method, text
        );
        if !status.is_success() {
            anyhow::bail!(
                "JDownloader API call {} returned {}: {}",
                method,
                status,
                text
            );
        }
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Invalid JSON from JDownloader ({}): {}", e, text))?;
        Ok(unwrap_value(value))
    }
}

/// JDownloader's deprecated API always wraps the response in an
/// `ObjectData`/`DataObject`: `{"data": <value>, "rid": -1, ...}`. This
/// function extracts the `data` field.
///
/// When the underlying value is Java `null` (e.g. a config key that was
/// never set), JDownloader serializes the whole envelope as a bare `{}`
/// with no `data` field at all, rather than `{"data": null, ...}` —
/// confirmed against a running instance. Without this, that case fell
/// through and returned the empty envelope object itself unchanged, which
/// callers expecting e.g. an array (`serde_json::from_value::<Vec<_>>`)
/// would then fail to deserialize.
pub(super) fn unwrap_value(mut value: Value) -> Value {
    if let Value::Object(ref mut m) = value {
        return m.remove("data").unwrap_or(Value::Null);
    }
    value
}
