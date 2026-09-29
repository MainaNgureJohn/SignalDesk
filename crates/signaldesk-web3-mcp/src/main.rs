#![deny(unsafe_code)]

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;
use signaldesk_agentic_wallet::AgenticWalletCli;
use signaldesk_web3::{load_stored_client, Web3Error};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ResearchParams {
    /// Exact underlying stock ticker, for example AAPL, NVDA, or MSFT.
    #[schemars(length(min = 1, max = 16))]
    ticker: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreviewBuyParams {
    /// Exact underlying stock ticker, for example AAPL, NVDA, or MSFT.
    #[schemars(length(min = 1, max = 16))]
    ticker: String,
    /// Human-readable BSC USDT amount, for example "5" or "25.50".
    #[schemars(length(min = 1, max = 32))]
    usdt_amount: String,
    /// BSC wallet address that would receive the bStock.
    #[schemars(length(min = 42, max = 42))]
    wallet_address: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgenticBuyParams {
    /// Exact underlying stock ticker, for example AAPL, NVDA, or MSFT.
    #[schemars(length(min = 1, max = 16))]
    ticker: String,
    /// Human-readable BSC USDT amount, for example "5" or "25.50".
    #[schemars(length(min = 1, max = 32))]
    usdt_amount: String,
    /// Slippage percentage, or "auto".
    #[schemars(length(min = 1, max = 16))]
    slippage: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExecuteAgenticBuyParams {
    /// Exact underlying stock ticker used in the quote.
    #[schemars(length(min = 1, max = 16))]
    ticker: String,
    /// Human-readable BSC USDT amount used in the quote.
    #[schemars(length(min = 1, max = 32))]
    usdt_amount: String,
    /// Slippage percentage, or "auto", used in the quote.
    #[schemars(length(min = 1, max = 16))]
    slippage: String,
    /// Exact fresh owner confirmation line copied from the current message.
    #[schemars(length(min = 1, max = 512))]
    confirmation: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OrderStatusParams {
    /// Agentic Wallet market order identifier returned by submission.
    #[schemars(length(min = 1, max = 128))]
    order_id: String,
}

#[derive(Clone)]
struct SignalDeskWeb3Mcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl SignalDeskWeb3Mcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "research_bsc_bstock",
        description = "Resolve one exact stock ticker to its BSC bStock contract and return current token price, underlying reference price, and reference-market status. This tool is read-only and never prepares, signs, or broadcasts a transaction.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn research_bsc_bstock(
        &self,
        Parameters(params): Parameters<ResearchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let client = match tokio::task::spawn_blocking(load_stored_client).await {
            Ok(Ok(client)) => client,
            Ok(Err(Web3Error::MissingCredentials)) => {
                return Ok(tool_error(
                    "Binance Web3 is not connected. Open SignalDesk Settings → Binance Web3 and save API credentials first.",
                ));
            }
            Ok(Err(error)) => return Ok(tool_error(error.to_string())),
            Err(error) => return Ok(tool_error(format!("credential task failed: {error}"))),
        };

        match client.research_bstock(&params.ticker).await {
            Ok(packet) => serde_json::to_string_pretty(&packet)
                .map(|json| CallToolResult::success(vec![Content::text(json)]))
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(error) => Ok(tool_error(error.to_string())),
        }
    }

    #[tool(
        name = "preview_bsc_bstock_buy",
        description = "Preview buying an exact BSC bStock ticker with a human-readable USDT amount for a specified wallet. Returns research plus the best short-lived RFQ quote. This tool is read-only and never builds calldata, requests a signature, approves tokens, or broadcasts a transaction.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn preview_bsc_bstock_buy(
        &self,
        Parameters(params): Parameters<PreviewBuyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let client = match tokio::task::spawn_blocking(load_stored_client).await {
            Ok(Ok(client)) => client,
            Ok(Err(Web3Error::MissingCredentials)) => {
                return Ok(tool_error(
                    "Binance Web3 is not connected. Open SignalDesk Settings → Binance Web3 and save API credentials first.",
                ));
            }
            Ok(Err(error)) => return Ok(tool_error(error.to_string())),
            Err(error) => return Ok(tool_error(format!("credential task failed: {error}"))),
        };

        match client
            .preview_bstock_buy(&params.ticker, &params.usdt_amount, &params.wallet_address)
            .await
        {
            Ok(preview) => serde_json::to_string_pretty(&preview)
                .map(|json| CallToolResult::success(vec![Content::text(json)]))
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(error) => Ok(tool_error(error.to_string())),
        }
    }

    #[tool(
        name = "get_agentic_wallet_status",
        description = "Return Binance Agentic Wallet connection status. This tool is read-only.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn get_agentic_wallet_status(&self) -> Result<CallToolResult, ErrorData> {
        agentic_result(AgenticWalletCli::discover(), |cli| async move {
            cli.status().await
        })
        .await
    }

    #[tool(
        name = "get_agentic_wallet_bsc_balances",
        description = "Return Agentic Wallet balances on BNB Smart Chain. This tool is read-only.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn get_agentic_wallet_bsc_balances(&self) -> Result<CallToolResult, ErrorData> {
        agentic_result(AgenticWalletCli::discover(), |cli| async move {
            cli.bsc_balances().await
        })
        .await
    }

    #[tool(
        name = "quote_agentic_wallet_bstock_buy",
        description = "Resolve an exact BSC bStock ticker through Binance Web3 and request a read-only USDT market-order quote from Binance Agentic Wallet. Reject an explicitly closed market; report unavailable market status honestly. No transaction is submitted.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn quote_agentic_wallet_bstock_buy(
        &self,
        Parameters(params): Parameters<AgenticBuyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let client = match load_web3_client().await {
            Ok(client) => client,
            Err(result) => return Ok(result),
        };
        let research = match client.research_bstock(&params.ticker).await {
            Ok(packet) => packet,
            Err(error) => return Ok(tool_error(error.to_string())),
        };
        if let Err(message) = reject_explicitly_closed_market(&research) {
            return Ok(tool_error(message));
        }
        let wallet = match AgenticWalletCli::discover() {
            Ok(wallet) => wallet,
            Err(error) => return Ok(tool_error(error.to_string())),
        };
        match wallet
            .quote_bsc_buy(
                &research.asset.token_contract_address,
                &params.usdt_amount,
                &params.slippage,
            )
            .await
        {
            Ok(quote) => json_success(serde_json::json!({
                "research": research,
                "agenticWalletQuote": quote,
                "requiredConfirmation": confirmation_line(
                    &params.ticker,
                    &research.asset.token_contract_address,
                    &params.usdt_amount,
                    &params.slippage,
                ),
            })),
            Err(error) => Ok(tool_error(error.to_string())),
        }
    }

    #[tool(
        name = "execute_agentic_wallet_bstock_buy",
        description = "Submit exactly one BSC bStock market buy through Binance Agentic Wallet. This is a state-changing on-chain action. It requires an exact owner authorization line matching the current contract and order, supplied as a standalone owner message. Re-quote before execution and return a submitted order identifier, not a completed-trade claim.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn execute_agentic_wallet_bstock_buy(
        &self,
        Parameters(params): Parameters<ExecuteAgenticBuyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if std::env::var("SIGNALDESK_WEB3_ALLOW_EXECUTION").as_deref() != Ok("1") {
            return Ok(tool_error(
                "Only the SignalDesk trading operator may execute wallet orders.",
            ));
        }
        let client = match load_web3_client().await {
            Ok(client) => client,
            Err(result) => return Ok(result),
        };
        let research = match client.research_bstock(&params.ticker).await {
            Ok(packet) => packet,
            Err(error) => return Ok(tool_error(error.to_string())),
        };
        if let Err(message) = reject_explicitly_closed_market(&research) {
            return Ok(tool_error(message));
        }
        let expected = confirmation_line(
            &params.ticker,
            &research.asset.token_contract_address,
            &params.usdt_amount,
            &params.slippage,
        );
        if params.confirmation != expected {
            return Ok(tool_error(format!(
                "Owner authorization did not exactly match the current contract and order. No order was submitted. Ask the owner to send this line in a new message: {expected}"
            )));
        }
        let wallet = match AgenticWalletCli::discover() {
            Ok(wallet) => wallet,
            Err(error) => return Ok(tool_error(error.to_string())),
        };
        if let Err(error) = wallet
            .quote_bsc_buy(
                &research.asset.token_contract_address,
                &params.usdt_amount,
                &params.slippage,
            )
            .await
        {
            return Ok(tool_error(format!(
                "A fresh Agentic Wallet quote failed, so no order was submitted: {error}"
            )));
        }
        match wallet
            .execute_bsc_buy(
                &research.asset.token_contract_address,
                &params.usdt_amount,
                &params.slippage,
            )
            .await
        {
            Ok(submission) => json_success(serde_json::json!({
                "state": "SUBMITTED_NOT_FINAL",
                "research": research,
                "agenticWalletSubmission": submission,
                "nextStep": "Poll get_agentic_wallet_market_order until FINISHED or FAILED before reporting the final outcome.",
            })),
            Err(error) => Ok(tool_error(error.to_string())),
        }
    }

    #[tool(
        name = "get_agentic_wallet_market_order",
        description = "Read one Binance Agentic Wallet market order by ID. FINISHED and FAILED are terminal; PENDING is not. This tool is read-only.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn get_agentic_wallet_market_order(
        &self,
        Parameters(params): Parameters<OrderStatusParams>,
    ) -> Result<CallToolResult, ErrorData> {
        agentic_result(AgenticWalletCli::discover(), |cli| async move {
            cli.order_status(&params.order_id).await
        })
        .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SignalDeskWeb3Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "signaldesk-web3-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "BSC tokenized-stock research plus Fizz-only Binance Agentic Wallet execution. Treat prices as time-sensitive. Never describe a preview or SUBMITTED order as an executed trade. If the owner's current message contains a standalone exact CONFIRM BINANCE ACTION line, it is one-time authorization even when it precedes the quote: obtain a fresh quote, verify the current contract, amount, and slippage match that line, then execute once without asking for another confirmation. Decimal spellings such as 5 and 5.0 represent the same USDT amount. An unavailable Binance Web3 market-status field is not proof the market is closed; disclose that status and use the fresh executable Agentic Wallet quote. Otherwise quote first, show the exact contract, amount, slippage, market status, and required confirmation line, then stop. Never execute on an explicitly closed market or a mismatch. Poll the returned order to FINISHED or FAILED and report the transaction hash only from the terminal result.",
            )
    }
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message.into())])
}

fn reject_explicitly_closed_market(
    research: &signaldesk_web3::RwaResearchPacket,
) -> Result<(), String> {
    if let Some(status) = research
        .market_status
        .as_ref()
        .filter(|status| !status.open_state)
    {
        return Err(format!(
            "{} is not currently open for trading: {}",
            research.ticker, status.market_status
        ));
    }
    Ok(())
}

async fn load_web3_client() -> Result<signaldesk_web3::Web3Client, CallToolResult> {
    match tokio::task::spawn_blocking(load_stored_client).await {
        Ok(Ok(client)) => Ok(client),
        Ok(Err(Web3Error::MissingCredentials)) => Err(tool_error(
            "Binance Web3 is not connected. Open SignalDesk Settings → Binance Web3 and save API credentials first.",
        )),
        Ok(Err(error)) => Err(tool_error(error.to_string())),
        Err(error) => Err(tool_error(format!("credential task failed: {error}"))),
    }
}

async fn agentic_result<F, Fut>(
    cli: Result<AgenticWalletCli, signaldesk_agentic_wallet::WalletError>,
    operation: F,
) -> Result<CallToolResult, ErrorData>
where
    F: FnOnce(AgenticWalletCli) -> Fut,
    Fut: std::future::Future<
        Output = Result<serde_json::Value, signaldesk_agentic_wallet::WalletError>,
    >,
{
    let cli = match cli {
        Ok(cli) => cli,
        Err(error) => return Ok(tool_error(error.to_string())),
    };
    match operation(cli).await {
        Ok(value) => json_success(value),
        Err(error) => Ok(tool_error(error.to_string())),
    }
}

fn json_success(value: serde_json::Value) -> Result<CallToolResult, ErrorData> {
    serde_json::to_string_pretty(&value)
        .map(|json| CallToolResult::success(vec![Content::text(json)]))
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))
}

fn confirmation_line(ticker: &str, contract: &str, amount: &str, slippage: &str) -> String {
    let amount = canonical_usdt_amount(amount);
    format!(
        "CONFIRM BINANCE ACTION: product=BSTOCK; action=BUY; symbol={}; contract={contract}; pay_asset=USDT; pay_amount={amount}; type=MARKET; slippage={slippage}",
        ticker.trim().to_ascii_uppercase()
    )
}

fn canonical_usdt_amount(amount: &str) -> String {
    let amount = amount.trim();
    if !amount.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.')
        || amount.bytes().filter(|byte| *byte == b'.').count() > 1
    {
        return amount.to_string();
    }
    let (whole, fraction) = amount.split_once('.').unwrap_or((amount, ""));
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let service = SignalDeskWeb3Mcp::new().serve(stdio()).await?;
            service.waiting().await?;
            Ok(())
        })
}
