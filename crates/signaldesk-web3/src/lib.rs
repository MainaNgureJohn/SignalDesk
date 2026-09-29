#![deny(unsafe_code)]
#![warn(missing_docs)]
//! Typed, authenticated access to the Binance Web3 API for SignalDesk.
//!
//! The client currently exposes read-only RWA discovery and market-data
//! endpoints. It deliberately fixes the API host and `/build` signing prefix so
//! credentials cannot be redirected to an arbitrary server.

use std::fmt;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{SecondsFormat, Utc};
use futures_util::StreamExt;
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::{Method, Request, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

const BASE_URL: &str = "https://web3.binance.com/build";
const SIGNED_PREFIX: &str = "/build";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
#[cfg(feature = "credential-store")]
const CREDENTIAL_SERVICE: &str = "app.signaldesk.binance-web3";
#[cfg(feature = "credential-store")]
const CREDENTIAL_ACCOUNT: &str = "api-credentials-v1";

const QUERY_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Binance's chain identifier for BNB Smart Chain.
pub const BSC_CHAIN_ID: &str = "56";
/// Binance-Peg USDT contract used as the funding token for BSC previews.
pub const BSC_USDT_CONTRACT: &str = "0x55d398326f99059fF775485246999027B3197955";
const BSC_USDT_DECIMALS: u32 = 18;
const QUOTE_TTL_SECONDS: u16 = 30;

/// Errors returned by the SignalDesk Binance Web3 client.
#[derive(Debug, thiserror::Error)]
pub enum Web3Error {
    /// API credentials were absent or blank.
    #[error("Binance Web3 API credentials are missing")]
    MissingCredentials,
    /// The operating-system credential store could not complete the operation.
    #[error("Binance Web3 credential store failed: {0}")]
    CredentialStore(String),
    /// A caller supplied an invalid value.
    #[error("invalid request: {0}")]
    InvalidInput(String),
    /// An exact BSC bStock ticker could not be resolved.
    #[error("no BSC bStock was found for ticker {0}")]
    AssetNotFound(String),
    /// An exact ticker resolved to more than one BSC bStock contract.
    #[error("more than one BSC bStock matched ticker {0}")]
    AmbiguousAsset(String),
    /// The price endpoint omitted the resolved contract.
    #[error("Binance Web3 returned no price for contract {0}")]
    PriceNotFound(String),
    /// The aggregator returned no route for a requested tokenized-stock trade.
    #[error("Binance Web3 returned no BSC swap quote for {0}")]
    QuoteNotFound(String),
    /// HMAC signing failed.
    #[error("could not sign Binance Web3 API request")]
    Signing,
    /// The HTTP request could not be completed.
    #[error("Binance Web3 API transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    /// The response exceeded the fixed client limit.
    #[error("Binance Web3 API response exceeded {MAX_RESPONSE_BYTES} bytes")]
    ResponseTooLarge,
    /// The API returned a non-successful HTTP status.
    #[error("Binance Web3 API returned HTTP {status}: {body}")]
    Http {
        /// HTTP response status.
        status: u16,
        /// Bounded response text.
        body: String,
    },
    /// The response was not valid JSON for the documented endpoint.
    #[error("invalid Binance Web3 API response: {0}")]
    Decode(#[from] serde_json::Error),
    /// Binance returned a non-zero business status code.
    #[error("Binance Web3 API error {code}: {message}")]
    Api {
        /// Binance business status code.
        code: i64,
        /// Binance error message.
        message: String,
    },
    /// A successful response omitted its data field.
    #[error("Binance Web3 API response did not include data")]
    MissingData,
}

/// Supported tokenized-stock issuance platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RwaPlatform {
    /// Binance bStocks.
    BStock,
    /// Ondo tokenized stocks.
    Ondo,
}

impl RwaPlatform {
    fn as_str(self) -> &'static str {
        match self {
            Self::BStock => "bstock",
            Self::Ondo => "ondo",
        }
    }
}

/// A tokenized asset returned for an underlying ticker.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RwaAsset {
    /// Issuance platform identifier.
    pub platform_id: String,
    /// Binance chain identifier, with `56` representing BSC.
    pub binance_chain_id: String,
    /// Onchain token contract address.
    pub token_contract_address: String,
    /// Token symbol used onchain.
    pub token_symbol: String,
    /// Binance RWA asset category.
    pub asset_type: i64,
}

/// A ticker match returned by RWA search.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RwaSearchHit {
    /// Underlying stock ticker.
    pub ticker: String,
    /// Underlying company name.
    pub company_name: String,
    /// Tokenized representations of the stock.
    pub assets: Vec<RwaAsset>,
}

/// Current issuance and reference-market state for an RWA token.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatus {
    /// Whether the underlying reference market is open.
    pub open_state: bool,
    /// Human-readable market session state.
    pub market_status: String,
    /// Machine-readable reason for a closed or restricted state.
    pub reason_code: Option<String>,
    /// Human-readable status reason.
    pub reason_msg: Option<String>,
    /// Next opening time as Unix milliseconds, when supplied.
    pub next_open_time: Option<i64>,
    /// Next closing time as Unix milliseconds, when supplied.
    pub next_close_time: Option<i64>,
}

/// Tokenized-stock summary returned by the RWA token list endpoint.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RwaToken {
    /// Binance chain identifier.
    pub binance_chain_id: String,
    /// Onchain contract address.
    pub token_contract_address: String,
    /// Issuance platform identifier.
    pub platform_id: String,
    /// Display name for the token.
    pub token_name: String,
    /// Onchain token symbol.
    pub token_symbol: String,
    /// Underlying stock ticker.
    pub underlying_ticker: String,
    /// Underlying company or asset name.
    pub underlying_name: String,
    /// Token decimals.
    pub decimals: u8,
    /// Issuance and reference-market status.
    pub status_info: MarketStatus,
    /// Latest onchain token price, represented exactly as returned.
    pub token_price: Option<String>,
    /// Latest underlying reference price, represented exactly as returned.
    pub reference_price: Option<String>,
    /// Reported 24-hour token volume.
    pub volume24_h: Option<String>,
}

/// Onchain and reference prices for an RWA token.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RwaPrice {
    /// Binance chain identifier.
    pub binance_chain_id: String,
    /// Onchain contract address.
    pub token_contract_address: String,
    /// Issuance platform identifier.
    pub platform_id: String,
    /// Latest onchain token price.
    pub token_price: String,
    /// Latest underlying reference price.
    pub reference_price: String,
    /// Token price update time as Unix milliseconds.
    pub token_price_updated_at: i64,
}

/// Market data for the underlying stock represented by an RWA token.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnderlyingMarket {
    /// Binance chain identifier.
    pub binance_chain_id: String,
    /// Onchain token contract address.
    pub token_contract_address: String,
    /// Issuance platform identifier.
    pub platform_id: String,
    /// Current reference-market status.
    pub status_info: MarketStatus,
    /// Market values whose decimal precision is preserved as strings.
    pub market_data: UnderlyingMarketData,
}

/// Reference-market values for an underlying stock.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnderlyingMarketData {
    /// Current underlying reference price.
    pub reference_price: String,
    /// 52-week high, when supplied.
    pub high52_w: Option<String>,
    /// 52-week low, when supplied.
    pub low52_w: Option<String>,
    /// Shares traded over the last 24 hours, when supplied.
    pub volume_shares24_h: Option<String>,
    /// One-year average daily volume, when supplied.
    pub avg_daily_volume1_y: Option<String>,
    /// Reported market capitalization, when supplied.
    pub market_cap: Option<String>,
    /// Trailing price-to-earnings ratio, when supplied.
    pub pe_ratio_ttm: Option<String>,
}

/// A single, exact BSC bStock research result assembled from Binance Web3.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RwaResearchPacket {
    /// Exact underlying ticker requested by the caller.
    pub ticker: String,
    /// Underlying company name returned by search.
    pub company_name: String,
    /// Resolved BSC bStock asset.
    pub asset: RwaAsset,
    /// Latest token and reference prices.
    pub price: RwaPrice,
    /// Current underlying-market data and session status, when available.
    pub underlying_market: Option<UnderlyingMarket>,
    /// Why supplemental underlying-market data could not be loaded.
    pub underlying_market_error: Option<String>,
    /// Verified RWA market status from the underlying-market or token-list endpoint.
    pub market_status: Option<MarketStatus>,
    /// Why no market status could be verified from either endpoint.
    pub market_status_error: Option<String>,
}

/// Token metadata attached to an aggregator quote.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuoteToken {
    /// Token contract address.
    pub token_contract_address: String,
    /// Token symbol.
    pub token_symbol: String,
    /// Current USD unit price.
    pub token_unit_price: String,
    /// Token decimals as returned by Binance.
    pub decimal: String,
    /// Whether Binance identifies the token as a honeypot.
    pub is_honey_pot: bool,
    /// Token transfer tax rate.
    pub tax_rate: String,
}

/// One route returned by the Binance Web3 DEX aggregator.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SwapQuote {
    /// Short-lived route identifier.
    pub quote_id: String,
    /// Aggregator or RFQ vendor.
    pub vendor_name: String,
    /// Binance chain identifier.
    pub binance_chain_id: String,
    /// Funding amount in the token's smallest unit.
    pub from_token_amount: String,
    /// Estimated output in the token's smallest unit.
    pub to_token_amount: String,
    /// Estimated trade fee in USD, when supplied.
    pub trade_fee: Option<String>,
    /// Estimated gas in the chain's smallest unit, when supplied.
    pub estimate_gas_fee: Option<String>,
    /// Estimated price impact percentage, when supplied.
    pub price_impact_percent: Option<String>,
    /// Funding-token metadata.
    pub from_token: QuoteToken,
    /// Output-token metadata.
    pub to_token: QuoteToken,
    /// `RFQ` for equity/RWA routes and `SWAP` for ordinary DEX routes.
    pub execution_mode: String,
    /// Token-approval spender, when an approval is required.
    pub approve_target: Option<String>,
    /// Whether Binance marked this as the best returned route.
    pub is_best: bool,
}

/// Read-only preview for buying one exact BSC bStock with USDT.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RwaTradePreview {
    /// Exact research packet used to resolve the destination contract.
    pub research: RwaResearchPacket,
    /// Human-readable USDT amount requested by the user.
    pub usdt_amount: String,
    /// USDT amount in the BSC token's smallest unit.
    pub usdt_amount_raw: String,
    /// Wallet address bound into the RFQ quote.
    pub wallet_address: String,
    /// Best route returned by Binance Web3.
    pub quote: SwapQuote,
    /// Documented approximate lifetime of the short-lived quote.
    pub quote_ttl_seconds: u16,
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    code: i64,
    msg: String,
    data: Option<T>,
}

/// Only the fields needed to verify a trade's exact contract and market state.
/// Other RWA response fields may be null or encoded inconsistently by Binance.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RwaStatusRecord {
    binance_chain_id: String,
    token_contract_address: String,
    status_info: Option<MarketStatus>,
}

/// Read-only Binance Web3 API client used by SignalDesk.
pub struct Web3Client {
    http: reqwest::Client,
    api_key: Zeroizing<String>,
    secret_key: Zeroizing<String>,
}

impl fmt::Debug for Web3Client {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Web3Client")
            .field("api_key", &"[redacted]")
            .field("secret_key", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl Web3Client {
    /// Creates a client from explicit credentials.
    pub fn new(
        api_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Result<Self, Web3Error> {
        let api_key = api_key.into();
        let secret_key = secret_key.into();
        if api_key.trim().is_empty() || secret_key.trim().is_empty() {
            return Err(Web3Error::MissingCredentials);
        }
        if api_key
            .bytes()
            .any(|byte| matches!(byte, b'\0' | b'\n' | b'\r'))
            || secret_key
                .bytes()
                .any(|byte| matches!(byte, b'\0' | b'\n' | b'\r'))
        {
            return Err(Web3Error::InvalidInput(
                "credentials cannot contain null bytes or line breaks".into(),
            ));
        }

        Ok(Self {
            http: reqwest::Client::new(),
            api_key: Zeroizing::new(api_key),
            secret_key: Zeroizing::new(secret_key),
        })
    }

    /// Searches BSC tokenized stocks by ticker, company name, or contract.
    pub async fn search_rwa(
        &self,
        keyword: &str,
        platform: Option<RwaPlatform>,
    ) -> Result<Vec<RwaSearchHit>, Web3Error> {
        if keyword.trim().is_empty() {
            return Err(Web3Error::InvalidInput(
                "search keyword cannot be blank".into(),
            ));
        }
        let mut query = vec![("keyword", keyword)];
        let platform_value;
        if let Some(platform) = platform {
            platform_value = platform.as_str();
            query.push(("platformId", platform_value));
        }
        let mut hits: Vec<RwaSearchHit> = self.get("/api/v1/dex/market/rwa/search", &query).await?;
        retain_bsc_assets(&mut hits);
        Ok(hits)
    }

    /// Lists tokenized stocks on BSC, optionally restricted to one platform.
    pub async fn list_rwa_tokens(
        &self,
        platform: Option<RwaPlatform>,
    ) -> Result<Vec<RwaToken>, Web3Error> {
        let mut query = vec![("binanceChainId", BSC_CHAIN_ID)];
        let platform_value;
        if let Some(platform) = platform {
            platform_value = platform.as_str();
            query.push(("platformId", platform_value));
        }
        self.get("/api/v1/dex/market/rwa/tokens", &query).await
    }

    /// Gets onchain and reference prices for BSC token contracts.
    pub async fn rwa_prices(&self, contracts: &[String]) -> Result<Vec<RwaPrice>, Web3Error> {
        if contracts.is_empty() || contracts.len() > 100 {
            return Err(Web3Error::InvalidInput(
                "price lookup requires between 1 and 100 contracts".into(),
            ));
        }
        if contracts.iter().any(|contract| contract.trim().is_empty()) {
            return Err(Web3Error::InvalidInput(
                "contract address cannot be blank".into(),
            ));
        }
        let addresses = contracts.join(",");
        self.get(
            "/api/v1/dex/market/rwa/price",
            &[
                ("binanceChainId", BSC_CHAIN_ID),
                ("tokenContractAddresses", addresses.as_str()),
            ],
        )
        .await
    }

    /// Gets the current underlying-market state for a BSC RWA token.
    pub async fn underlying_market(&self, contract: &str) -> Result<UnderlyingMarket, Web3Error> {
        if contract.trim().is_empty() {
            return Err(Web3Error::InvalidInput(
                "contract address cannot be blank".into(),
            ));
        }
        self.get(
            "/api/v1/dex/market/rwa/underlying-market",
            &[
                ("binanceChainId", BSC_CHAIN_ID),
                ("tokenContractAddress", contract),
            ],
        )
        .await
    }

    /// Resolves an exact ticker and builds a read-only BSC bStock research packet.
    pub async fn research_bstock(&self, ticker: &str) -> Result<RwaResearchPacket, Web3Error> {
        let ticker = normalize_ticker(ticker)?;
        let hits = self.search_rwa(&ticker, Some(RwaPlatform::BStock)).await?;
        let (company_name, asset) = select_exact_bstock(&ticker, hits)?;
        let contract = asset.token_contract_address.clone();
        let contracts = [contract.clone()];
        let prices = self.rwa_prices(&contracts).await?;
        let price = prices
            .into_iter()
            .find(|price| {
                price
                    .token_contract_address
                    .eq_ignore_ascii_case(contract.as_str())
            })
            .ok_or_else(|| Web3Error::PriceNotFound(contract.clone()))?;
        let (underlying_market, underlying_market_error) =
            match self.underlying_market(&contract).await {
                Ok(market) => (Some(market), None),
                Err(error) => (None, Some(error.to_string())),
            };
        let (market_status, market_status_error) = if let Some(market) = &underlying_market {
            (Some(market.status_info.clone()), None)
        } else {
            let status_from_underlying: Result<RwaStatusRecord, Web3Error> = self
                .get(
                    "/api/v1/dex/market/rwa/underlying-market",
                    &[
                        ("binanceChainId", BSC_CHAIN_ID),
                        ("tokenContractAddress", &contract),
                    ],
                )
                .await;
            let verified = status_from_underlying.ok().and_then(|record| {
                (record.binance_chain_id == BSC_CHAIN_ID
                    && record
                        .token_contract_address
                        .eq_ignore_ascii_case(&contract))
                .then_some(record.status_info)
                .flatten()
            });
            if let Some(status) = verified {
                (Some(status), None)
            } else {
                let token_list: Result<Vec<RwaStatusRecord>, Web3Error> = self
                    .get(
                        "/api/v1/dex/market/rwa/tokens",
                        &[("binanceChainId", BSC_CHAIN_ID), ("platformId", "bstock")],
                    )
                    .await;
                match token_list {
                    Ok(tokens) => {
                        let status = tokens.into_iter().find_map(|record| {
                            (record.binance_chain_id == BSC_CHAIN_ID
                                && record
                                    .token_contract_address
                                    .eq_ignore_ascii_case(&contract))
                            .then_some(record.status_info)
                            .flatten()
                        });
                        let error = status.is_none().then(|| {
                            format!(
                                "Binance RWA token list did not include market status for {ticker}"
                            )
                        });
                        (status, error)
                    }
                    Err(error) => (None, Some(error.to_string())),
                }
            }
        };

        Ok(RwaResearchPacket {
            ticker,
            company_name,
            asset,
            price,
            underlying_market,
            underlying_market_error,
            market_status,
            market_status_error,
        })
    }

    /// Builds a read-only USDT-to-bStock quote for an exact ticker and wallet.
    ///
    /// This does not build calldata, request a signature, or broadcast a transaction.
    pub async fn preview_bstock_buy(
        &self,
        ticker: &str,
        usdt_amount: &str,
        wallet_address: &str,
    ) -> Result<RwaTradePreview, Web3Error> {
        validate_evm_address(wallet_address)?;
        let usdt_amount = usdt_amount.trim();
        let usdt_amount_raw = decimal_to_units(usdt_amount, BSC_USDT_DECIMALS)?;
        let research = self.research_bstock(ticker).await?;
        let destination = research.asset.token_contract_address.as_str();
        let quotes: Vec<SwapQuote> = self
            .get(
                "/api/v1/dex/aggregator/quote",
                &[
                    ("binanceChainId", BSC_CHAIN_ID),
                    ("amount", usdt_amount_raw.as_str()),
                    ("fromTokenAddress", BSC_USDT_CONTRACT),
                    ("toTokenAddress", destination),
                    ("userWalletAddress", wallet_address),
                ],
            )
            .await?;
        let quote = quotes
            .iter()
            .find(|quote| quote.is_best)
            .cloned()
            .or_else(|| quotes.into_iter().next())
            .ok_or_else(|| Web3Error::QuoteNotFound(research.ticker.clone()))?;

        if quote.binance_chain_id != BSC_CHAIN_ID
            || !quote
                .from_token
                .token_contract_address
                .eq_ignore_ascii_case(BSC_USDT_CONTRACT)
            || !quote
                .to_token
                .token_contract_address
                .eq_ignore_ascii_case(destination)
        {
            return Err(Web3Error::InvalidInput(
                "Binance Web3 quote did not match the requested BSC pair".into(),
            ));
        }

        Ok(RwaTradePreview {
            research,
            usdt_amount: usdt_amount.to_owned(),
            usdt_amount_raw,
            wallet_address: wallet_address.to_owned(),
            quote,
            quote_ttl_seconds: QUOTE_TTL_SECONDS,
        })
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, Web3Error> {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let request = self.build_get_request(path, query, &timestamp)?;
        let response = self.http.execute(request).await?;
        decode_response(response).await
    }

    fn build_get_request(
        &self,
        path: &str,
        query: &[(&str, &str)],
        timestamp: &str,
    ) -> Result<Request, Web3Error> {
        if !path.starts_with("/api/") || path.contains('?') || path.contains('#') {
            return Err(Web3Error::InvalidInput("endpoint path is invalid".into()));
        }
        let path_and_query = encode_query(path, query);
        let signed_path = format!("{SIGNED_PREFIX}{path_and_query}");
        let signature = sign(
            self.secret_key.as_bytes(),
            timestamp,
            &Method::GET,
            &signed_path,
            "",
        )?;

        self.http
            .get(format!("{BASE_URL}{path_and_query}"))
            .header("X-OC-APIKEY", self.api_key.as_str())
            .header("X-OC-TIMESTAMP", timestamp)
            .header("X-OC-RECV-WINDOW", "60000")
            .header("X-OC-SIGN", signature)
            .build()
            .map_err(Web3Error::Transport)
    }
}

fn validate_evm_address(address: &str) -> Result<(), Web3Error> {
    let bytes = address.as_bytes();
    if bytes.len() != 42
        || !address.starts_with("0x")
        || !bytes[2..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(Web3Error::InvalidInput(
            "wallet address must be a 20-byte 0x-prefixed EVM address".into(),
        ));
    }
    Ok(())
}

fn decimal_to_units(amount: &str, decimals: u32) -> Result<String, Web3Error> {
    let amount = amount.trim();
    let (whole, fraction) = amount.split_once('.').unwrap_or((amount, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > decimals as usize
    {
        return Err(Web3Error::InvalidInput(
            "USDT amount must be a positive decimal with at most 18 decimal places".into(),
        ));
    }
    let scale = 10_u128.pow(decimals);
    let whole = whole.parse::<u128>().map_err(|_| {
        Web3Error::InvalidInput("USDT amount is outside the supported range".into())
    })?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u128>().map_err(|_| {
            Web3Error::InvalidInput("USDT amount is outside the supported range".into())
        })? * 10_u128.pow(decimals - fraction.len() as u32)
    };
    let units = whole
        .checked_mul(scale)
        .and_then(|value| value.checked_add(fraction))
        .ok_or_else(|| {
            Web3Error::InvalidInput("USDT amount is outside the supported range".into())
        })?;
    if units == 0 || units > 1_000_000_u128 * scale {
        return Err(Web3Error::InvalidInput(
            "USDT amount must be greater than 0 and no more than 1,000,000".into(),
        ));
    }
    Ok(units.to_string())
}

/// Stores validated Binance Web3 credentials in a dedicated OS credential entry.
#[cfg(feature = "credential-store")]
pub fn store_credentials(api_key: &str, secret_key: &str) -> Result<(), Web3Error> {
    Web3Client::new(api_key, secret_key)?;
    let encoded = encode_credentials(api_key, secret_key);
    credential_entry()?
        .set_password(&encoded)
        .map_err(|error| Web3Error::CredentialStore(error.to_string()))
}

/// Reports whether the dedicated OS credential entry is present and valid.
#[cfg(feature = "credential-store")]
pub fn credentials_present() -> Result<bool, Web3Error> {
    match credential_entry()?.get_password() {
        Ok(encoded) => {
            let encoded = Zeroizing::new(encoded);
            decode_credentials(&encoded)?;
            Ok(true)
        }
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(error) => Err(Web3Error::CredentialStore(error.to_string())),
    }
}

/// Loads a Binance Web3 client from the dedicated OS credential entry.
#[cfg(feature = "credential-store")]
pub fn load_stored_client() -> Result<Web3Client, Web3Error> {
    let encoded = credential_entry()?
        .get_password()
        .map_err(|error| match error {
            keyring::Error::NoEntry => Web3Error::MissingCredentials,
            other => Web3Error::CredentialStore(other.to_string()),
        })?;
    let encoded = Zeroizing::new(encoded);
    let (api_key, secret_key) = decode_credentials(&encoded)?;
    Web3Client::new(api_key.as_str(), secret_key.as_str())
}

/// Removes Binance Web3 credentials from the dedicated OS credential entry.
#[cfg(feature = "credential-store")]
pub fn delete_credentials() -> Result<(), Web3Error> {
    match credential_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(Web3Error::CredentialStore(error.to_string())),
    }
}

#[cfg(feature = "credential-store")]
fn credential_entry() -> Result<keyring::Entry, Web3Error> {
    keyring::Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_ACCOUNT)
        .map_err(|error| Web3Error::CredentialStore(error.to_string()))
}

#[cfg(any(feature = "credential-store", test))]
fn encode_credentials(api_key: &str, secret_key: &str) -> Zeroizing<String> {
    Zeroizing::new(format!("{api_key}\n{secret_key}"))
}

#[cfg(any(feature = "credential-store", test))]
fn decode_credentials(encoded: &str) -> Result<(Zeroizing<String>, Zeroizing<String>), Web3Error> {
    let (api_key, secret_key) = encoded
        .split_once('\n')
        .ok_or_else(|| Web3Error::CredentialStore("saved credential is incomplete".into()))?;
    Web3Client::new(api_key, secret_key)?;
    Ok((
        Zeroizing::new(api_key.to_owned()),
        Zeroizing::new(secret_key.to_owned()),
    ))
}

fn normalize_ticker(ticker: &str) -> Result<String, Web3Error> {
    let ticker = ticker.trim();
    if ticker.is_empty()
        || ticker.len() > 16
        || !ticker
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err(Web3Error::InvalidInput(
            "ticker must be 1-16 ASCII letters, numbers, dots, or hyphens".into(),
        ));
    }
    Ok(ticker.to_ascii_uppercase())
}

fn select_exact_bstock(
    ticker: &str,
    hits: Vec<RwaSearchHit>,
) -> Result<(String, RwaAsset), Web3Error> {
    let mut matches = hits.into_iter().flat_map(|hit| {
        let underlying_matches = hit.ticker.eq_ignore_ascii_case(ticker);
        hit.assets
            .into_iter()
            .filter(move |asset| {
                asset.binance_chain_id == BSC_CHAIN_ID
                    && asset.platform_id.eq_ignore_ascii_case("bstock")
                    && (underlying_matches || asset.token_symbol.eq_ignore_ascii_case(ticker))
            })
            .map(move |asset| (hit.company_name.clone(), asset))
    });
    let first = matches
        .next()
        .ok_or_else(|| Web3Error::AssetNotFound(ticker.to_owned()))?;
    if matches.next().is_some() {
        return Err(Web3Error::AmbiguousAsset(ticker.to_owned()));
    }
    Ok(first)
}

fn retain_bsc_assets(hits: &mut Vec<RwaSearchHit>) {
    for hit in hits.iter_mut() {
        hit.assets
            .retain(|asset| asset.binance_chain_id == BSC_CHAIN_ID);
    }
    hits.retain(|hit| !hit.assets.is_empty());
}

fn encode_query(path: &str, query: &[(&str, &str)]) -> String {
    if query.is_empty() {
        return path.to_owned();
    }
    let encoded = query
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                utf8_percent_encode(key, QUERY_ENCODE_SET),
                utf8_percent_encode(value, QUERY_ENCODE_SET)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{encoded}")
}

fn sign(
    secret: &[u8],
    timestamp: &str,
    method: &Method,
    signed_path: &str,
    body: &str,
) -> Result<String, Web3Error> {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(secret).map_err(|_| Web3Error::Signing)?;
    mac.update(timestamp.as_bytes());
    mac.update(method.as_str().as_bytes());
    mac.update(signed_path.as_bytes());
    mac.update(body.as_bytes());
    Ok(STANDARD.encode(mac.finalize().into_bytes()))
}

async fn decode_response<T: DeserializeOwned>(response: Response) -> Result<T, Web3Error> {
    let status = response.status();
    let bytes = read_bounded(response).await?;
    if !status.is_success() {
        return Err(Web3Error::Http {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }
    let envelope: ApiEnvelope<T> = serde_json::from_slice(&bytes)?;
    if envelope.code != 0 {
        return Err(Web3Error::Api {
            code: envelope.code,
            message: envelope.msg,
        });
    }
    envelope.data.ok_or(Web3Error::MissingData)
}

async fn read_bounded(response: Response) -> Result<Vec<u8>, Web3Error> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(Web3Error::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(Web3Error::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_request_matches_wire_path_and_redacts_credentials() {
        let client = Web3Client::new("api-key", "test-secret").unwrap();
        let request = client
            .build_get_request(
                "/api/v1/dex/market/rwa/search",
                &[("keyword", "NVIDIA Corp"), ("platformId", "bstock")],
                "2026-05-11T10:08:57.715Z",
            )
            .unwrap();

        assert_eq!(
            request.url().as_str(),
            "https://web3.binance.com/build/api/v1/dex/market/rwa/search?keyword=NVIDIA%20Corp&platformId=bstock"
        );
        assert_eq!(request.headers()["X-OC-APIKEY"], "api-key");
        assert_eq!(request.headers()["X-OC-RECV-WINDOW"], "60000");
        assert_eq!(
            request.headers()["X-OC-SIGN"],
            "tyn5OrVd0jA6iYbRy5F04EDq3HGtQ99SWJUqTdN/HT4="
        );
        let debug = format!("{client:?}");
        assert!(!debug.contains("api-key"));
        assert!(!debug.contains("test-secret"));
    }

    #[test]
    fn parses_documented_rwa_search_shape() {
        let json = br#"{
            "code": 0,
            "msg": "success",
            "data": [{
                "ticker": "NVDA",
                "companyName": "NVIDIA Corporation",
                "assets": [{
                    "platformId": "bstock",
                    "binanceChainId": "56",
                    "tokenContractAddress": "0x1234",
                    "tokenSymbol": "NVDAb",
                    "assetType": 1
                }]
            }]
        }"#;
        let envelope: ApiEnvelope<Vec<RwaSearchHit>> = serde_json::from_slice(json).unwrap();
        let hits = envelope.data.unwrap();
        assert_eq!(hits[0].ticker, "NVDA");
        assert_eq!(hits[0].assets[0].binance_chain_id, BSC_CHAIN_ID);
        assert_eq!(hits[0].assets[0].platform_id, "bstock");
    }

    #[test]
    fn search_results_keep_only_bsc_assets() {
        let mut hits = vec![RwaSearchHit {
            ticker: "NVDA".into(),
            company_name: "NVIDIA Corporation".into(),
            assets: vec![
                RwaAsset {
                    platform_id: "bstock".into(),
                    binance_chain_id: BSC_CHAIN_ID.into(),
                    token_contract_address: "0xbsc".into(),
                    token_symbol: "NVDAb".into(),
                    asset_type: 1,
                },
                RwaAsset {
                    platform_id: "other".into(),
                    binance_chain_id: "1".into(),
                    token_contract_address: "0xeth".into(),
                    token_symbol: "NVDAe".into(),
                    asset_type: 1,
                },
            ],
        }];

        retain_bsc_assets(&mut hits);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].assets.len(), 1);
        assert_eq!(hits[0].assets[0].binance_chain_id, BSC_CHAIN_ID);
    }

    #[test]
    fn exact_token_symbol_resolves_underlying_ticker_without_fuzzy_matching() {
        let hit = RwaSearchHit {
            ticker: "SPCX".into(),
            company_name: "SpaceX".into(),
            assets: vec![RwaAsset {
                platform_id: "bstock".into(),
                binance_chain_id: BSC_CHAIN_ID.into(),
                token_contract_address: "0xbe9d156892e55e7154bcd3cb0fea677f9d3103e1".into(),
                token_symbol: "SPCXB".into(),
                asset_type: 1,
            }],
        };
        assert!(select_exact_bstock("SPCXB", vec![hit.clone()]).is_ok());
        assert!(matches!(
            select_exact_bstock("SPC", vec![hit]),
            Err(Web3Error::AssetNotFound(_))
        ));
    }

    #[test]
    fn status_record_ignores_unrelated_nullable_and_string_fields() {
        let record: RwaStatusRecord = serde_json::from_str(
            r#"{"binanceChainId":"56","tokenContractAddress":"0x123","decimals":"18","marketData":{"referencePrice":null},"statusInfo":{"openState":true,"marketStatus":"regular","reasonCode":null,"reasonMsg":null,"nextOpenTime":null,"nextCloseTime":null}}"#,
        )
        .unwrap();
        assert!(record.status_info.unwrap().open_state);
    }

    #[test]
    fn rejects_blank_credentials_and_invalid_contract_sets() {
        assert!(matches!(
            Web3Client::new("", "secret"),
            Err(Web3Error::MissingCredentials)
        ));
        let client = Web3Client::new("api-key", "secret").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(matches!(
            runtime.block_on(client.rwa_prices(&[])),
            Err(Web3Error::InvalidInput(_))
        ));
    }

    #[test]
    fn exact_bstock_selection_rejects_fuzzy_and_ambiguous_results() {
        let asset = |contract: &str| RwaAsset {
            platform_id: "bstock".into(),
            binance_chain_id: BSC_CHAIN_ID.into(),
            token_contract_address: contract.into(),
            token_symbol: "AAPLb".into(),
            asset_type: 1,
        };
        let hit = |ticker: &str, assets: Vec<RwaAsset>| RwaSearchHit {
            ticker: ticker.into(),
            company_name: "Apple Inc.".into(),
            assets,
        };

        let (_, selected) = select_exact_bstock(
            "AAPL",
            vec![
                hit("AAP", vec![asset("0xfuzzy")]),
                hit("aapl", vec![asset("0xexact")]),
            ],
        )
        .unwrap();
        assert_eq!(selected.token_contract_address, "0xexact");

        assert!(matches!(
            select_exact_bstock("AAPL", vec![hit("AAPL", vec![asset("0x1"), asset("0x2")])]),
            Err(Web3Error::AmbiguousAsset(_))
        ));
        assert!(matches!(
            select_exact_bstock("MSFT", vec![hit("AAPL", vec![asset("0x1")])]),
            Err(Web3Error::AssetNotFound(_))
        ));
        assert_eq!(normalize_ticker(" brk.b ").unwrap(), "BRK.B");
        assert!(normalize_ticker("AAPL/USDT").is_err());
    }

    #[test]
    fn converts_human_usdt_without_floating_point() {
        assert_eq!(decimal_to_units("5", 18).unwrap(), "5000000000000000000");
        assert_eq!(decimal_to_units("0.25", 18).unwrap(), "250000000000000000");
        assert_eq!(
            decimal_to_units("1.000000000000000001", 18).unwrap(),
            "1000000000000000001"
        );
        for invalid in ["0", "-1", ".5", "1.0000000000000000001", "1e3", "1,000"] {
            assert!(decimal_to_units(invalid, 18).is_err(), "accepted {invalid}");
        }
        assert!(decimal_to_units("1000000", 18).is_ok());
        assert!(decimal_to_units("1000001", 18).is_err());
    }

    #[test]
    fn validates_bsc_wallet_addresses() {
        assert!(validate_evm_address("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").is_ok());
        assert!(validate_evm_address("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045").is_err());
        assert!(validate_evm_address("0xnot-an-address").is_err());
    }

    #[test]
    fn parses_documented_aggregator_quote_shape() {
        let json = br#"{
            "code": 0,
            "msg": "success",
            "data": [{
                "quoteId": "quote-1",
                "vendorName": "PcsXRfq",
                "binanceChainId": "56",
                "fromTokenAmount": "5000000000000000000",
                "toTokenAmount": "125000000000000000",
                "tradeFee": "0.12",
                "estimateGasFee": "150000",
                "priceImpactPercent": "-0.01",
                "fromToken": {
                    "tokenContractAddress": "0x55d398326f99059fF775485246999027B3197955",
                    "tokenSymbol": "USDT",
                    "tokenUnitPrice": "1.00",
                    "decimal": "18",
                    "isHoneyPot": false,
                    "taxRate": "0"
                },
                "toToken": {
                    "tokenContractAddress": "0x1234567890123456789012345678901234567890",
                    "tokenSymbol": "AAPLb",
                    "tokenUnitPrice": "200.00",
                    "decimal": "18",
                    "isHoneyPot": false,
                    "taxRate": "0"
                },
                "executionMode": "RFQ",
                "approveTarget": "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
                "isBest": true
            }]
        }"#;
        let envelope: ApiEnvelope<Vec<SwapQuote>> = serde_json::from_slice(json).unwrap();
        let quote = envelope.data.unwrap().remove(0);
        assert_eq!(quote.execution_mode, "RFQ");
        assert_eq!(quote.from_token.token_symbol, "USDT");
        assert!(quote.is_best);
    }

    #[test]
    fn credential_codec_round_trips_and_rejects_line_breaks() {
        let encoded = encode_credentials("api-key", "secret-key");
        let (api_key, secret_key) = decode_credentials(&encoded).unwrap();
        assert_eq!(api_key.as_str(), "api-key");
        assert_eq!(secret_key.as_str(), "secret-key");
        assert!(matches!(
            Web3Client::new("api\nkey", "secret"),
            Err(Web3Error::InvalidInput(_))
        ));
        assert!(decode_credentials("api-key-only").is_err());
    }
}
