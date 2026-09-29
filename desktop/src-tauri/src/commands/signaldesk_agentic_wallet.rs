use serde::Serialize;
use signaldesk_agentic_wallet_pkg::AgenticWalletCli;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgenticWalletStatus {
    installed: bool,
    version: Option<String>,
    connection_status: Option<String>,
}

/// Reports whether the Agentic Wallet CLI is installed and connected.
#[tauri::command]
pub async fn get_signaldesk_agentic_wallet_status() -> Result<AgenticWalletStatus, String> {
    let cli = match AgenticWalletCli::discover() {
        Ok(cli) => cli,
        Err(_) => {
            return Ok(AgenticWalletStatus {
                installed: false,
                version: None,
                connection_status: None,
            });
        }
    };
    let version = cli.version().await.ok();
    let status = cli.status().await.map_err(|error| error.to_string())?;
    Ok(AgenticWalletStatus {
        installed: true,
        version,
        connection_status: status
            .pointer("/data/status")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    })
}

/// Reads BNB Smart Chain balances from the connected wallet without signing.
#[tauri::command]
pub async fn get_signaldesk_agentic_wallet_bsc_balances() -> Result<serde_json::Value, String> {
    AgenticWalletCli::discover()
        .map_err(|error| error.to_string())?
        .bsc_balances()
        .await
        .map_err(|error| error.to_string())
}

/// Starts Binance's QR/link sign-in flow and returns its public pairing data.
#[tauri::command]
pub async fn start_signaldesk_agentic_wallet_sign_in() -> Result<serde_json::Value, String> {
    AgenticWalletCli::discover()
        .map_err(|error| error.to_string())?
        .sign_in()
        .await
        .map_err(|error| error.to_string())
}

/// Keeps the official verification process alive while the user confirms in Binance App.
#[tauri::command]
pub async fn verify_signaldesk_agentic_wallet_sign_in(
    qr_code_id: String,
) -> Result<AgenticWalletStatus, String> {
    let cli = AgenticWalletCli::discover().map_err(|error| error.to_string())?;
    cli.verify_sign_in(&qr_code_id)
        .await
        .map_err(|error| error.to_string())?;
    get_signaldesk_agentic_wallet_status().await
}

/// Signs the local Agentic Wallet CLI session out.
#[tauri::command]
pub async fn sign_out_signaldesk_agentic_wallet() -> Result<AgenticWalletStatus, String> {
    AgenticWalletCli::discover()
        .map_err(|error| error.to_string())?
        .sign_out()
        .await
        .map_err(|error| error.to_string())?;
    get_signaldesk_agentic_wallet_status().await
}
