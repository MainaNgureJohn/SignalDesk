use serde::Serialize;
use signaldesk_web3_pkg::{
    credentials_present, delete_credentials, load_stored_client, store_credentials,
    RwaResearchPacket, Web3Client,
};
use zeroize::Zeroizing;

const CONNECTION_TEST_TICKER: &str = "AAPL";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalDeskWeb3Status {
    configured: bool,
    verified: bool,
    bsc_bstock_matches: usize,
}

/// Reports whether Binance Web3 credentials are present without returning them.
#[tauri::command]
pub async fn get_signaldesk_web3_status() -> Result<SignalDeskWeb3Status, String> {
    tokio::task::spawn_blocking(|| {
        if credentials_present().map_err(|error| error.to_string())? {
            Ok(SignalDeskWeb3Status {
                configured: true,
                verified: false,
                bsc_bstock_matches: 0,
            })
        } else {
            Ok(SignalDeskWeb3Status {
                configured: false,
                verified: false,
                bsc_bstock_matches: 0,
            })
        }
    })
    .await
    .map_err(|error| format!("credential status task failed: {error}"))?
}

/// Tests and saves Binance Web3 credentials in the operating-system keyring.
#[tauri::command]
pub async fn save_signaldesk_web3_credentials(
    api_key: String,
    secret_key: String,
) -> Result<SignalDeskWeb3Status, String> {
    let api_key = Zeroizing::new(api_key);
    let secret_key = Zeroizing::new(secret_key);
    let client = Web3Client::new(api_key.as_str(), secret_key.as_str())
        .map_err(|error| error.to_string())?;
    client
        .research_bstock(CONNECTION_TEST_TICKER)
        .await
        .map_err(|error| error.to_string())?;
    tokio::task::spawn_blocking(move || store_credentials(&api_key, &secret_key))
        .await
        .map_err(|error| format!("credential save task failed: {error}"))?
        .map_err(|error| error.to_string())?;

    Ok(SignalDeskWeb3Status {
        configured: true,
        verified: true,
        bsc_bstock_matches: 1,
    })
}

/// Tests the saved credential against Binance's read-only RWA search endpoint.
#[tauri::command]
pub async fn test_signaldesk_web3_connection() -> Result<SignalDeskWeb3Status, String> {
    let client = load_client().await?;
    client
        .research_bstock(CONNECTION_TEST_TICKER)
        .await
        .map_err(|error| error.to_string())?;
    Ok(SignalDeskWeb3Status {
        configured: true,
        verified: true,
        bsc_bstock_matches: 1,
    })
}

/// Builds an exact, read-only BSC bStock research packet for the interface.
#[tauri::command]
pub async fn research_signaldesk_bstock(ticker: String) -> Result<RwaResearchPacket, String> {
    load_client()
        .await?
        .research_bstock(&ticker)
        .await
        .map_err(|error| error.to_string())
}

/// Removes the saved Binance Web3 credential from protected storage.
#[tauri::command]
pub async fn delete_signaldesk_web3_credentials() -> Result<(), String> {
    tokio::task::spawn_blocking(delete_credentials)
        .await
        .map_err(|error| format!("credential delete task failed: {error}"))?
        .map_err(|error| error.to_string())
}

async fn load_client() -> Result<Web3Client, String> {
    tokio::task::spawn_blocking(load_stored_client)
        .await
        .map_err(|error| format!("credential load task failed: {error}"))?
        .map_err(|error| error.to_string())
}
