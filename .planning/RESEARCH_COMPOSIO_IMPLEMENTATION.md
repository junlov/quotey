# Composio REST API Implementation Research

**Integration:** CRM via Composio  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Builds On:** RCH-06, RCH-07, ADR-RCH07

---

## 1. Executive Summary

This document provides Rust-specific implementation guidance for Composio REST API integration, building on the architectural decisions and patterns established by prior research (RCH-06, RCH-07, ADR-RCH07).

**Key Implementation Decisions:**
- Use `reqwest` for HTTP client with connection pooling
- Implement `CrmAdapter` trait with `StubCrmAdapter` and `ComposioCrmAdapter`
- Async/await throughout with proper error handling
- Idempotent operations with correlation tracking

---

## 2. Composio API Fundamentals

### 2.1 Base Configuration

```rust
pub struct ComposioConfig {
    pub base_url: String,           // https://api.composio.dev
    pub api_key: SecretString,      // x-api-key header
    pub timeout_secs: u64,          // 30s default
    pub max_retries: u32,           // 3 default
    pub retry_base_ms: u64,         // 250ms base
}

impl Default for ComposioConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.composio.dev".to_string(),
            api_key: SecretString::new(String::new().into()),
            timeout_secs: 30,
            max_retries: 3,
            retry_base_ms: 250,
        }
    }
}
```

### 2.2 Authentication

```rust
pub struct ComposioAuth {
    api_key: SecretString,
}

impl ComposioAuth {
    pub fn new(api_key: SecretString) -> Self {
        Self { api_key }
    }
    
    pub fn apply_headers(&self, headers: &mut HeaderMap) {
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(self.api_key.expose_secret())
                .expect("API key is valid header value"),
        );
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );
    }
}
```

### 2.3 Core API Endpoints

```rust
pub struct ComposioEndpoints {
    base: String,
}

impl ComposioEndpoints {
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }
    
    // Auth Configs
    pub fn auth_configs(&self) -> String {
        format!("{}/v1/auth-configs", self.base)
    }
    
    pub fn auth_config(&self, config_id: &str) -> String {
        format!("{}/v1/auth-configs/{}", self.base, config_id)
    }
    
    // Connected Accounts
    pub fn connected_accounts(&self) -> String {
        format!("{}/v1/connected-accounts", self.base)
    }
    
    pub fn connected_account(&self, account_id: &str) -> String {
        format!("{}/v1/connected-accounts/{}", self.base, account_id)
    }
    
    // Tool Execution
    pub fn execute_tool(&self) -> String {
        format!("{}/v1/actions/execute", self.base)
    }
    
    // Tool Discovery
    pub fn tool_schemas(&self) -> String {
        format!("{}/v1/actions/schemas", self.base)
    }
    
    pub fn tool_schema(&self, tool_name: &str) -> String {
        format!("{}/v1/actions/{}/schema", self.base, tool_name)
    }
}
```

---

## 3. HTTP Client Implementation

### 3.1 Reqwest Client Configuration

```rust
pub struct ComposioClient {
    http: Client,
    auth: ComposioAuth,
    endpoints: ComposioEndpoints,
    config: ComposioConfig,
}

impl ComposioClient {
    pub fn new(config: ComposioConfig) -> Result<Self, ClientError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(ClientError::HttpBuild)?;
        
        let auth = ComposioAuth::new(config.api_key.clone());
        let endpoints = ComposioEndpoints::new(&config.base_url);
        
        Ok(Self {
            http,
            auth,
            endpoints,
            config,
        })
    }
    
    /// Execute request with retry logic
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<T, ComposioError> {
        let mut last_error = None;
        
        for attempt in 0..self.config.max_retries {
            match self.execute_request(method.clone(), url, body.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) if e.is_retryable() && attempt < self.config.max_retries - 1 => {
                    let delay = self.calculate_backoff(attempt);
                    tracing::warn!(
                        "Composio request failed (attempt {}/{}), retrying in {:?}: {}",
                        attempt + 1,
                        self.config.max_retries,
                        delay,
                        e
                    );
                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        
        Err(last_error.unwrap_or_else(|| ComposioError::MaxRetriesExceeded))
    }
    
    async fn execute_request<T: DeserializeOwned>(
        &self,
        method: Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<T, ComposioError> {
        let mut headers = HeaderMap::new();
        self.auth.apply_headers(&mut headers);
        
        // Add correlation ID if available
        if let Some(correlation_id) = tracing::Span::current()
            .context::<CorrelationId>()
            .map(|c| c.to_string()) {
            headers.insert(
                "x-correlation-id",
                HeaderValue::from_str(&correlation_id).unwrap_or_default(),
            );
        }
        
        let mut request = self.http.request(method, url).headers(headers);
        
        if let Some(body) = body {
            request = request.json(&body);
        }
        
        let response = request.send().await.map_err(ComposioError::Http)?;
        
        // Check rate limits
        if let Some(retry_after) = response.headers().get("retry-after") {
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                let delay = retry_after
                    .to_str()
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_secs(60));
                
                return Err(ComposioError::RateLimited { retry_after: delay });
            }
        }
        
        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(ComposioError::ApiError {
                status,
                message: error_body,
            });
        }
        
        // Parse response
        let data = response.json::<T>().await.map_err(ComposioError::Http)?;
        Ok(data)
    }
    
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base = self.config.retry_base_ms;
        let delay = base * 2_u64.pow(attempt);
        let jitter = rand::random::<u64>() % (delay / 4);
        Duration::from_millis(delay + jitter)
    }
}
```

### 3.2 Error Types

```rust
#[derive(Debug, Error)]
pub enum ComposioError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("HTTP client build error: {0}")]
    HttpBuild(reqwest::Error),
    
    #[error("API error (status {status}): {message}")]
    ApiError { status: StatusCode, message: String },
    
    #[error("Rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },
    
    #[error("Authentication failed")]
    AuthFailed,
    
    #[error("Connected account not found")]
    AccountNotFound,
    
    #[error("Tool execution failed: {0}")]
    ToolExecution(String),
    
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl ComposioError {
    /// Determine if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            Self::Http(e) if e.is_timeout() || e.is_connect() || 
                matches!(e.status(), Some(s) if s.is_server_error() && s != StatusCode::NOT_IMPLEMENTED)
            | Self::RateLimited { .. }
            | Self::ApiError { status, .. } if status.is_server_error()
        )
    }
    
    /// Map to application error
    pub fn into_application_error(self, correlation_id: &str) -> ApplicationError {
        match &self {
            Self::RateLimited { .. } => ApplicationError::Integration {
                service: "composio".to_string(),
                message: "Rate limited, will retry".to_string(),
            },
            Self::AuthFailed => ApplicationError::Configuration {
                message: "Composio authentication failed".to_string(),
            },
            _ => ApplicationError::Integration {
                service: "composio".to_string(),
                message: self.to_string(),
            },
        }
    }
}
```

---

## 4. Tool Execution API

### 4.1 Execute Tool Request/Response

```rust
#[derive(Debug, Serialize)]
pub struct ExecuteToolRequest {
    pub action_name: String,
    pub params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteToolResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<ToolExecutionError>,
}

#[derive(Debug, Deserialize)]
pub struct ToolExecutionError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub details: Option<Value>,
}
```

### 4.2 Tool Execution Client Methods

```rust
impl ComposioClient {
    /// Execute a Composio tool
    pub async fn execute_tool(
        &self,
        action: &str,
        params: Value,
        connected_account: Option<&str>,
    ) -> Result<Value, ComposioError> {
        let request = ExecuteToolRequest {
            action_name: action.to_string(),
            params,
            connected_account_id: connected_account.map(String::from),
            app_name: None,
        };
        
        let response: ExecuteToolResponse = self
            .request(Method::POST, &self.endpoints.execute_tool(), Some(json!(request)))
            .await?;
        
        if response.success {
            response.data.ok_or_else(|| {
                ComposioError::ToolExecution("Tool succeeded but returned no data".to_string())
            })
        } else {
            let error = response.error.ok_or_else(|| {
                ComposioError::ToolExecution("Tool failed with no error details".to_string())
            })?;
            Err(ComposioError::ToolExecution(format!(
                "{}: {}",
                error.error_type, error.message
            )))
        }
    }
    
    /// Execute with specific app context
    pub async fn execute_app_tool(
        &self,
        app: &str,
        action: &str,
        params: Value,
    ) -> Result<Value, ComposioError> {
        let request = ExecuteToolRequest {
            action_name: action.to_string(),
            params,
            connected_account_id: None,
            app_name: Some(app.to_string()),
        };
        
        let response: ExecuteToolResponse = self
            .request(Method::POST, &self.endpoints.execute_tool(), Some(json!(request)))
            .await?;
        
        if response.success {
            response.data.ok_or_else(|| {
                ComposioError::ToolExecution("Tool succeeded but returned no data".to_string())
            })
        } else {
            let error = response.error.unwrap();
            Err(ComposioError::ToolExecution(format!(
                "{}: {}",
                error.error_type, error.message
            )))
        }
    }
}
```

---

## 5. Connected Account Management

### 5.1 Account Types

```rust
#[derive(Debug, Deserialize)]
pub struct ConnectedAccount {
    pub id: String,
    pub app_name: String,
    pub status: AccountStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "appUniqueId")]
    pub app_unique_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Active,
    Inactive,
    Error,
}

#[derive(Debug, Serialize)]
pub struct CreateConnectionRequest {
    pub app_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_config_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConnectionResponse {
    pub id: String,
    pub app_name: String,
    pub connection_url: String,
    pub status: AccountStatus,
}
```

### 5.2 Account Management Methods

```rust
impl ComposioClient {
    /// List connected accounts
    pub async fn list_connected_accounts(
        &self,
    ) -> Result<Vec<ConnectedAccount>, ComposioError> {
        self.request(Method::GET, &self.endpoints.connected_accounts(), None)
            .await
    }
    
    /// Get specific connected account
    pub async fn get_connected_account(
        &self,
        account_id: &str,
    ) -> Result<ConnectedAccount, ComposioError> {
        self.request(
            Method::GET,
            &self.endpoints.connected_account(account_id),
            None,
        )
        .await
    }
    
    /// Initiate new connection
    pub async fn create_connection(
        &self,
        app: &str,
        redirect_url: Option<&str>,
    ) -> Result<CreateConnectionResponse, ComposioError> {
        let request = CreateConnectionRequest {
            app_name: app.to_string(),
            auth_config_id: None,
            redirect_url: redirect_url.map(String::from),
        };
        
        self.request(
            Method::POST,
            &self.endpoints.connected_accounts(),
            Some(json!(request)),
        )
        .await
    }
    
    /// Delete connection
    pub async fn delete_connection(
        &self,
        account_id: &str,
    ) -> Result<(), ComposioError> {
        self.request::<Value>(
            Method::DELETE,
            &self.endpoints.connected_account(account_id),
            None,
        )
        .await?;
        Ok(())
    }
}
```

---

## 6. CRM Adapter Implementation

### 6.1 CrmAdapter Trait

```rust
#[async_trait]
pub trait CrmAdapter: Send + Sync {
    /// Lookup account by name or domain
    async fn lookup_account(
        &self,
        name_or_domain: &str,
    ) -> Result<Option<CrmAccountDto>, CrmError>;
    
    /// Get deal by ID
    async fn get_deal(&self, deal_id: &str) -> Result<Option<CrmDealDto>, CrmError>;
    
    /// Create new deal
    async fn create_deal(
        &self,
        account_id: &str,
        deal_data: CreateDealRequest,
    ) -> Result<CrmDealDto, CrmError>;
    
    /// Update existing deal
    async fn update_deal(
        &self,
        deal_id: &str,
        updates: DealUpdates,
    ) -> Result<CrmDealDto, CrmError>;
    
    /// Write quote back to CRM
    async fn write_quote(
        &self,
        deal_id: &str,
        quote_summary: QuoteWriteDto,
    ) -> Result<QuoteWriteResult, CrmError>;
    
    /// Search contacts
    async fn search_contacts(
        &self,
        query: &str,
    ) -> Result<Vec<CrmContactDto>, CrmError>;
    
    /// Incremental sync
    async fn sync_incremental(&self, cursor: Option<&str>) -> Result<CrmSyncResult, CrmError>;
}

/// Canonical DTOs (vendor-neutral)
#[derive(Debug, Clone)]
pub struct CrmAccountDto {
    pub id: String,
    pub crm_ref: String,
    pub name: String,
    pub domain: Option<String>,
    pub segment: String,
    pub source_provider: String,
    pub source_ts: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CrmDealDto {
    pub id: String,
    pub crm_ref: String,
    pub account_id: String,
    pub name: String,
    pub stage: String,
    pub amount: Option<Decimal>,
    pub close_date: Option<NaiveDate>,
    pub source_provider: String,
    pub source_ts: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CrmContactDto {
    pub id: String,
    pub crm_ref: String,
    pub account_id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}
```

### 6.2 ComposioCrmAdapter Implementation

```rust
pub struct ComposioCrmAdapter {
    client: Arc<ComposioClient>,
    connected_account: String,
    app: String,  // "salesforce" or "hubspot"
    mapper: Arc<dyn CrmDataMapper>,
}

#[async_trait]
impl CrmAdapter for ComposioCrmAdapter {
    async fn lookup_account(
        &self,
        name_or_domain: &str,
    ) -> Result<Option<CrmAccountDto>, CrmError> {
        // Determine if lookup is by domain or name
        let (action, params) = if name_or_domain.contains('.') {
            // Domain lookup
            (
                "search_accounts",
                json!({
                    "domain": name_or_domain,
                    "limit": 5
                }),
            )
        } else {
            // Name lookup
            (
                "search_accounts",
                json!({
                    "name": name_or_domain,
                    "limit": 5
                }),
            )
        };
        
        let result = self
            .client
            .execute_tool(action, params, Some(&self.connected_account))
            .await
            .map_err(CrmError::from)?;
        
        // Map to canonical DTO
        let accounts: Vec<CrmAccountDto> = self.mapper.map_accounts(&result)?;
        Ok(accounts.into_iter().next())
    }
    
    async fn get_deal(&self, deal_id: &str) -> Result<Option<CrmDealDto>, CrmError> {
        let params = json!({
            "id": deal_id
        });
        
        let result = self
            .client
            .execute_tool("get_deal", params, Some(&self.connected_account))
            .await
            .map_err(CrmError::from)?;
        
        self.mapper.map_deal(&result).map(Some)
    }
    
    async fn create_deal(
        &self,
        account_id: &str,
        deal_data: CreateDealRequest,
    ) -> Result<CrmDealDto, CrmError> {
        let params = json!({
            "account_id": account_id,
            "name": deal_data.name,
            "stage": deal_data.stage,
            "amount": deal_data.amount,
            "close_date": deal_data.close_date.map(|d| d.to_string()),
        });
        
        let result = self
            .client
            .execute_tool("create_deal", params, Some(&self.connected_account))
            .await
            .map_err(CrmError::from)?;
        
        self.mapper.map_deal(&result)
    }
    
    async fn update_deal(
        &self,
        deal_id: &str,
        updates: DealUpdates,
    ) -> Result<CrmDealDto, CrmError> {
        let params = json!({
            "id": deal_id,
            "updates": {
                "stage": updates.stage,
                "amount": updates.amount,
                "close_date": updates.close_date.map(|d| d.to_string()),
            }
        });
        
        let result = self
            .client
            .execute_tool("update_deal", params, Some(&self.connected_account))
            .await
            .map_err(CrmError::from)?;
        
        self.mapper.map_deal(&result)
    }
    
    async fn write_quote(
        &self,
        deal_id: &str,
        quote_summary: QuoteWriteDto,
    ) -> Result<QuoteWriteResult, CrmError> {
        let params = json!({
            "deal_id": deal_id,
            "quote_id": quote_summary.quote_id,
            "version": quote_summary.version,
            "total": quote_summary.total.to_string(),
            "currency": quote_summary.currency,
            "status": quote_summary.status,
            "valid_until": quote_summary.valid_until.map(|d| d.to_string()),
            "attachment_ref": quote_summary.attachment_ref,
        });
        
        let result = self
            .client
            .execute_tool("create_quote", params, Some(&self.connected_account))
            .await
            .map_err(CrmError::from)?;
        
        Ok(QuoteWriteResult {
            crm_quote_id: result["id"].as_str().unwrap_or_default().to_string(),
            deal_id: deal_id.to_string(),
            synced_at: Utc::now(),
        })
    }
    
    async fn search_contacts(
        &self,
        query: &str,
    ) -> Result<Vec<CrmContactDto>, CrmError> {
        let params = json!({
            "query": query,
            "limit": 20
        });
        
        let result = self
            .client
            .execute_tool("search_contacts", params, Some(&self.connected_account))
            .await
            .map_err(CrmError::from)?;
        
        self.mapper.map_contacts(&result)
    }
    
    async fn sync_incremental(&self, cursor: Option<&str>) -> Result<CrmSyncResult, CrmError> {
        let params = if let Some(cursor) = cursor {
            json!({
                "cursor": cursor,
                "limit": 100
            })
        } else {
            json!({
                "limit": 100
            })
        };
        
        let result = self
            .client
            .execute_tool("get_recent_changes", params, Some(&self.connected_account))
            .await
            .map_err(CrmError::from)?;
        
        Ok(CrmSyncResult {
            accounts: self.mapper.map_accounts(&result["accounts"])?,
            deals: self.mapper.map_deals(&result["deals"])?,
            contacts: self.mapper.map_contacts(&result["contacts"])?,
            next_cursor: result["next_cursor"].as_str().map(String::from),
            has_more: result["has_more"].as_bool().unwrap_or(false),
        })
    }
}
```

---

## 7. Data Mapping Layer

### 7.1 Salesforce Mapper

```rust
pub struct SalesforceMapper;

impl CrmDataMapper for SalesforceMapper {
    fn map_accounts(&self, data: &Value) -> Result<Vec<CrmAccountDto>, MappingError> {
        let records = data["records"]
            .as_array()
            .ok_or(MappingError::UnexpectedFormat)?;
        
        records
            .iter()
            .map(|r| {
                Ok(CrmAccountDto {
                    id: Uuid::new_v4().to_string(),
                    crm_ref: r["Id"].as_str().unwrap_or_default().to_string(),
                    name: r["Name"].as_str().unwrap_or_default().to_string(),
                    domain: r["Website"].as_str().map(|s| {
                        s.replace("https://", "").replace("http://", "")
                    }),
                    segment: r["Type"].as_str().unwrap_or("Unknown").to_string(),
                    source_provider: "salesforce".to_string(),
                    source_ts: Utc::now(),
                })
            })
            .collect()
    }
    
    fn map_deal(&self, data: &Value) -> Result<CrmDealDto, MappingError> {
        Ok(CrmDealDto {
            id: Uuid::new_v4().to_string(),
            crm_ref: data["Id"].as_str().unwrap_or_default().to_string(),
            account_id: data["AccountId"].as_str().unwrap_or_default().to_string(),
            name: data["Name"].as_str().unwrap_or_default().to_string(),
            stage: data["StageName"].as_str().unwrap_or_default().to_string(),
            amount: data["Amount"]
                .as_f64()
                .map(|a| Decimal::try_from(a).ok())
                .flatten(),
            close_date: data["CloseDate"]
                .as_str()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
            source_provider: "salesforce".to_string(),
            source_ts: Utc::now(),
        })
    }
    
    fn map_contacts(&self, data: &Value) -> Result<Vec<CrmContactDto>, MappingError> {
        let records = data["records"]
            .as_array()
            .ok_or(MappingError::UnexpectedFormat)?;
        
        records
            .iter()
            .map(|r| {
                Ok(CrmContactDto {
                    id: Uuid::new_v4().to_string(),
                    crm_ref: r["Id"].as_str().unwrap_or_default().to_string(),
                    account_id: r["AccountId"].as_str().unwrap_or_default().to_string(),
                    name: format!(
                        "{} {}",
                        r["FirstName"].as_str().unwrap_or(""),
                        r["LastName"].as_str().unwrap_or("")
                    )
                    .trim()
                    .to_string(),
                    email: r["Email"].as_str().map(String::from),
                    phone: r["Phone"].as_str().map(String::from),
                })
            })
            .collect()
    }
}
```

### 7.2 HubSpot Mapper

```rust
pub struct HubSpotMapper;

impl CrmDataMapper for HubSpotMapper {
    fn map_accounts(&self, data: &Value) -> Result<Vec<CrmAccountDto>, MappingError> {
        let results = data["results"]
            .as_array()
            .ok_or(MappingError::UnexpectedFormat)?;
        
        results
            .iter()
            .map(|r| {
                let props = &r["properties"];
                Ok(CrmAccountDto {
                    id: Uuid::new_v4().to_string(),
                    crm_ref: r["id"].as_str().unwrap_or_default().to_string(),
                    name: props["name"]["value"].as_str().unwrap_or_default().to_string(),
                    domain: props["domain"]["value"].as_str().map(String::from),
                    segment: props["type"]["value"].as_str().unwrap_or("Unknown").to_string(),
                    source_provider: "hubspot".to_string(),
                    source_ts: Utc::now(),
                })
            })
            .collect()
    }
    
    // ... similar implementations for deals and contacts
}
```

---

## 8. Stub Adapter for Offline Mode

```rust
pub struct StubCrmAdapter {
    fixtures: Arc<Mutex<CrmFixtures>>,
    delay_ms: u64,  // Simulate network latency
}

#[async_trait]
impl CrmAdapter for StubCrmAdapter {
    async fn lookup_account(
        &self,
        name_or_domain: &str,
    ) -> Result<Option<CrmAccountDto>, CrmError> {
        self.simulate_delay().await;
        
        let fixtures = self.fixtures.lock().await;
        let account = fixtures.accounts.iter()
            .find(|a| {
                a.name.to_lowercase().contains(&name_or_domain.to_lowercase())
                    || a.domain.as_ref().map(|d| d.contains(name_or_domain)).unwrap_or(false)
            })
            .cloned();
        
        Ok(account)
    }
    
    async fn get_deal(&self, deal_id: &str) -> Result<Option<CrmDealDto>, CrmError> {
        self.simulate_delay().await;
        
        let fixtures = self.fixtures.lock().await;
        let deal = fixtures.deals.iter()
            .find(|d| d.crm_ref == deal_id || d.id == deal_id)
            .cloned();
        
        Ok(deal)
    }
    
    async fn create_deal(
        &self,
        account_id: &str,
        deal_data: CreateDealRequest,
    ) -> Result<CrmDealDto, CrmError> {
        self.simulate_delay().await;
        
        let deal = CrmDealDto {
            id: Uuid::new_v4().to_string(),
            crm_ref: format!("STUB-DEAL-{}", rand::random::<u16>()),
            account_id: account_id.to_string(),
            name: deal_data.name,
            stage: deal_data.stage,
            amount: deal_data.amount,
            close_date: deal_data.close_date,
            source_provider: "stub".to_string(),
            source_ts: Utc::now(),
        };
        
        self.fixtures.lock().await.deals.push(deal.clone());
        Ok(deal)
    }
    
    // ... other stub implementations
    
    async fn sync_incremental(&self, _cursor: Option<&str>) -> Result<CrmSyncResult, CrmError> {
        self.simulate_delay().await;
        
        let fixtures = self.fixtures.lock().await;
        Ok(CrmSyncResult {
            accounts: fixtures.accounts.clone(),
            deals: fixtures.deals.clone(),
            contacts: fixtures.contacts.clone(),
            next_cursor: None,
            has_more: false,
        })
    }
}

impl StubCrmAdapter {
    async fn simulate_delay(&self) {
        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
    }
}
```

---

## 9. Testing

### 9.1 Mock Server with wiremock

```rust
#[cfg(test)]
mod tests {
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};
    
    async fn setup_mock_composio() -> (MockServer, ComposioClient) {
        let server = MockServer::start().await;
        
        let config = ComposioConfig {
            base_url: server.uri(),
            api_key: SecretString::new("test-key".into()),
            ..Default::default()
        };
        
        let client = ComposioClient::new(config).unwrap();
        (server, client)
    }
    
    #[tokio::test]
    async fn test_execute_tool_success() {
        let (server, client) = setup_mock_composio().await;
        
        Mock::given(method("POST"))
            .and(path("/v1/actions/execute"))
            .and(header("x-api-key", "test-key"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "success": true,
                    "data": {
                        "Id": "001XXXXXXXXXXXX",
                        "Name": "Acme Corp"
                    }
                })))
            .mount(&server)
            .await;
        
        let result = client
            .execute_tool("get_account", json!({"id": "001XXXXXXXXXXXX"}), None)
            .await;
        
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data["Name"], "Acme Corp");
    }
    
    #[tokio::test]
    async fn test_rate_limit_retry() {
        let (server, client) = setup_mock_composio().await;
        
        // First call returns 429
        Mock::given(method("POST"))
            .and(path("/v1/actions/execute"))
            .respond_with(ResponseTemplate::new(429)
                .insert_header("retry-after", "1"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        
        // Second call succeeds
        Mock::given(method("POST"))
            .and(path("/v1/actions/execute"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({"success": true, "data": {}})))
            .mount(&server)
            .await;
        
        let start = Instant::now();
        let result = client.execute_tool("test", json!({}), None).await;
        let elapsed = start.elapsed();
        
        assert!(result.is_ok());
        assert!(elapsed >= Duration::from_secs(1));  // Waited for retry-after
    }
}
```

---

## 10. Configuration

```toml
# quotey.toml
[crm]
provider = "composio"  # or "stub"

[crm.composio]
api_key = "${COMPOSIO_API_KEY}"
base_url = "https://api.composio.dev"
timeout_secs = 30
max_retries = 3

# Provider-specific settings
[crm.composio.salesforce]
connected_account_id = "${SALESFORCE_ACCOUNT_ID}"

[crm.composio.hubspot]
connected_account_id = "${HUBSPOT_ACCOUNT_ID}"

[crm.stub]
fixtures_path = "config/crm/stub"
simulate_delay_ms = 100
```

---

## 11. References

1. Composio API Docs: https://docs.composio.dev/reference
2. Reqwest Documentation: https://docs.rs/reqwest
3. Prior Research: RCH-06, RCH-07, ADR-RCH07

---

*Implementation research compiled by ResearchAgent for the quotey project.*
