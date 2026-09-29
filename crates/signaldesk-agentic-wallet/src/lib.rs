#![deny(unsafe_code)]

use std::{path::PathBuf, process::Stdio, time::Duration};

use serde_json::Value;
use thiserror::Error;
use tokio::{io::AsyncReadExt, process::Command, time::timeout};

const MAX_OUTPUT_BYTES: u64 = 256 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Error)]
/// Failures reported while validating input, launching the official wallet
/// CLI, or decoding its bounded response.
pub enum WalletError {
    #[error("Binance Agentic Wallet CLI is not installed. Install it with: npm install -g @binance/agentic-wallet@1.10.0")]
    NotInstalled,
    #[error("invalid Agentic Wallet input: {0}")]
    InvalidInput(String),
    #[error("Agentic Wallet command timed out")]
    Timeout,
    #[error("Agentic Wallet command output exceeded 256 KiB")]
    OutputTooLarge,
    #[error("Agentic Wallet command failed: {0}")]
    Command(String),
    #[error("Agentic Wallet returned invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Agentic Wallet process failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
/// Validated, bounded adapter around the official Binance Agentic Wallet CLI.
pub struct AgenticWalletCli {
    command: PathBuf,
}

impl AgenticWalletCli {
    /// Locates the official `baw` command on the current process `PATH`.
    pub fn discover() -> Result<Self, WalletError> {
        discover_baw().map(|command| Self { command })
    }

    #[cfg(test)]
    fn with_command(command: PathBuf) -> Self {
        Self { command }
    }

    /// Returns the installed Agentic Wallet CLI version.
    pub async fn version(&self) -> Result<String, WalletError> {
        let output = self.run(&["--version"], DEFAULT_TIMEOUT).await?;
        Ok(output.trim().to_string())
    }

    /// Returns the redacted wallet connection status.
    pub async fn status(&self) -> Result<Value, WalletError> {
        self.run_json(&["wallet", "status", "--json"], DEFAULT_TIMEOUT)
            .await
    }

    /// Returns the redacted wallet balances on BNB Smart Chain.
    pub async fn bsc_balances(&self) -> Result<Value, WalletError> {
        self.run_json(
            &["wallet", "balance", "--binanceChainId", "56", "--json"],
            DEFAULT_TIMEOUT,
        )
        .await
    }

    /// Requests a read-only BSC USDT-to-token market-order quote.
    pub async fn quote_bsc_buy(
        &self,
        to_token: &str,
        usdt_amount: &str,
        slippage: &str,
    ) -> Result<Value, WalletError> {
        validate_address(to_token)?;
        validate_decimal(usdt_amount, "USDT amount")?;
        validate_slippage(slippage)?;
        self.run_json(
            &[
                "market-order",
                "quote",
                "--fromTokenQty",
                usdt_amount,
                "--fromToken",
                signaldesk_web3_usdt(),
                "--toToken",
                to_token,
                "--binanceChainId",
                "56",
                "--slippage",
                slippage,
                "--json",
            ],
            DEFAULT_TIMEOUT,
        )
        .await
    }

    /// Submits one BSC USDT-to-token market swap.
    pub async fn execute_bsc_buy(
        &self,
        to_token: &str,
        usdt_amount: &str,
        slippage: &str,
    ) -> Result<Value, WalletError> {
        validate_address(to_token)?;
        validate_decimal(usdt_amount, "USDT amount")?;
        validate_slippage(slippage)?;
        self.run_json(
            &[
                "market-order",
                "swap",
                "--fromTokenQty",
                usdt_amount,
                "--fromToken",
                signaldesk_web3_usdt(),
                "--toToken",
                to_token,
                "--binanceChainId",
                "56",
                "--slippage",
                slippage,
                "--mev",
                "true",
                "--gasLevel",
                "MEDIUM",
                "--json",
            ],
            DEFAULT_TIMEOUT,
        )
        .await
    }

    /// Returns the current state of one Agentic Wallet market order.
    pub async fn order_status(&self, order_id: &str) -> Result<Value, WalletError> {
        validate_identifier(order_id, "order id")?;
        self.run_json(
            &["market-order", "list", "--orderId", order_id, "--json"],
            DEFAULT_TIMEOUT,
        )
        .await
    }

    /// Starts the official Binance App wallet-pairing flow.
    pub async fn sign_in(&self) -> Result<Value, WalletError> {
        self.run_json(&["auth", "signin", "--json"], DEFAULT_TIMEOUT)
            .await
    }

    /// Waits for Binance App to confirm the specified pairing request.
    pub async fn verify_sign_in(&self, qr_code_id: &str) -> Result<Value, WalletError> {
        validate_identifier(qr_code_id, "QR code id")?;
        self.run_json(
            &["auth", "verify", "--qrCodeId", qr_code_id, "--json"],
            Duration::from_secs(310),
        )
        .await
    }

    /// Signs the local Agentic Wallet CLI session out.
    pub async fn sign_out(&self) -> Result<Value, WalletError> {
        self.run_json(&["auth", "signout", "--json"], DEFAULT_TIMEOUT)
            .await
    }

    async fn run_json(&self, args: &[&str], deadline: Duration) -> Result<Value, WalletError> {
        let output = self.run(args, deadline).await?;
        let mut value: Value = serde_json::from_str(output.trim())?;
        redact_sensitive(&mut value);
        if value.get("success").and_then(Value::as_bool) == Some(false) {
            return Err(WalletError::Command(value.to_string()));
        }
        Ok(value)
    }

    async fn run(&self, args: &[&str], deadline: Duration) -> Result<String, WalletError> {
        let mut command = command_for(&self.command, args);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                WalletError::NotInstalled
            } else {
                WalletError::Io(error)
            }
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| WalletError::Command("could not capture command output".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| WalletError::Command("could not capture command errors".to_string()))?;
        let stdout_task = tokio::spawn(read_bounded(stdout));
        let stderr_task = tokio::spawn(read_bounded(stderr));
        let status = match timeout(deadline, child.wait()).await {
            Ok(result) => result?,
            Err(_) => {
                let _ = child.kill().await;
                return Err(WalletError::Timeout);
            }
        };
        let stdout = stdout_task
            .await
            .map_err(|error| WalletError::Command(format!("output task failed: {error}")))??;
        let stderr = stderr_task
            .await
            .map_err(|error| WalletError::Command(format!("error task failed: {error}")))??;
        if !status.success() {
            return Err(WalletError::Command(sanitize_text(&stderr)));
        }
        String::from_utf8(stdout)
            .map_err(|_| WalletError::Command("command output was not UTF-8".to_string()))
    }
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(reader: R) -> Result<Vec<u8>, WalletError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() as u64 > MAX_OUTPUT_BYTES {
        return Err(WalletError::OutputTooLarge);
    }
    Ok(bytes)
}

fn discover_baw() -> Result<PathBuf, WalletError> {
    let candidates: &[&str] = if cfg!(windows) {
        &["baw.exe", "baw.cmd", "baw.bat"]
    } else {
        &["baw"]
    };
    let path = std::env::var_os("PATH").ok_or(WalletError::NotInstalled)?;
    for directory in std::env::split_paths(&path) {
        for name in candidates {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(WalletError::NotInstalled)
}

fn command_for(program: &std::path::Path, args: &[&str]) -> Command {
    #[cfg(windows)]
    if matches!(
        program.extension().and_then(|value| value.to_str()),
        Some("cmd" | "bat")
    ) {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/s", "/c"]);
        command.arg(program);
        command.args(args);
        command.env("BINANCE_INSTANCE_ID", stable_wallet_instance_id());
        return command;
    }
    let mut command = Command::new(program);
    command.args(args);
    command.env("BINANCE_INSTANCE_ID", stable_wallet_instance_id());
    command
}

fn stable_wallet_instance_id() -> String {
    // The Binance CLI otherwise derives its encryption key from the first
    // network adapter's MAC address. A VPN or adapter-order change makes it
    // regenerate the client ID and clear the authenticated wallet session.
    let user = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .or_else(|| std::env::var_os("USERNAME"))
        .unwrap_or_else(|| "local-user".into());
    format!(
        "signaldesk-agentic-wallet-v1:{}",
        user.to_string_lossy().to_ascii_lowercase()
    )
}

fn validate_address(value: &str) -> Result<(), WalletError> {
    if value.len() == 42
        && value.starts_with("0x")
        && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(WalletError::InvalidInput(
            "invalid BSC contract address".into(),
        ))
    }
}

fn validate_decimal(value: &str, label: &str) -> Result<(), WalletError> {
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    let valid = !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction
            .is_none_or(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        && parts.next().is_none()
        && value.bytes().any(|byte| matches!(byte, b'1'..=b'9'));
    if valid && value.len() <= 32 {
        Ok(())
    } else {
        Err(WalletError::InvalidInput(format!("invalid {label}")))
    }
}

fn validate_slippage(value: &str) -> Result<(), WalletError> {
    if value == "auto" {
        return Ok(());
    }
    validate_decimal(value, "slippage")?;
    let numeric = value
        .parse::<f64>()
        .map_err(|_| WalletError::InvalidInput("invalid slippage".into()))?;
    if numeric <= 100.0 {
        Ok(())
    } else {
        Err(WalletError::InvalidInput(
            "slippage must be at most 100".into(),
        ))
    }
}

fn validate_identifier(value: &str, label: &str) -> Result<(), WalletError> {
    if !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(WalletError::InvalidInput(format!("invalid {label}")))
    }
}

fn redact_sensitive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let lower = key.to_ascii_lowercase();
                if is_sensitive_key(&lower) {
                    *child = Value::String("[REDACTED]".into());
                } else {
                    redact_sensitive(child);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(redact_sensitive),
        _ => {}
    }
}

fn is_sensitive_key(value: &str) -> bool {
    value.contains("sessiontoken")
        || value.contains("accesstoken")
        || value.contains("refreshtoken")
        || value.contains("privatekey")
        || value.contains("seedphrase")
        || value == "clientid"
}

fn sanitize_text(value: &[u8]) -> String {
    let text = String::from_utf8_lossy(value);
    if let Ok(mut json) = serde_json::from_str::<Value>(&text) {
        redact_sensitive(&mut json);
        return json.to_string().chars().take(4_096).collect();
    }

    text.lines()
        .map(|line| {
            let normalized: String = line
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect();
            if [
                "sessiontoken",
                "accesstoken",
                "refreshtoken",
                "privatekey",
                "seedphrase",
                "clientid",
            ]
            .iter()
            .any(|key| normalized.contains(key))
            {
                "[REDACTED]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(4_096)
        .collect()
}

const fn signaldesk_web3_usdt() -> &'static str {
    "0x55d398326f99059fF775485246999027B3197955"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_inputs_without_shell_metacharacters() {
        assert!(validate_address("0x1111111111111111111111111111111111111111").is_ok());
        assert!(validate_address("0x1 & whoami").is_err());
        assert!(validate_decimal("5.25", "amount").is_ok());
        assert!(validate_decimal("5 & whoami", "amount").is_err());
        assert!(validate_slippage("auto").is_ok());
        assert!(validate_slippage("101").is_err());
        assert!(validate_identifier("a191884d-0e05-435b-a887-336bc242fafc", "id").is_ok());
    }

    #[test]
    fn redacts_sensitive_json_fields() {
        let mut value = serde_json::json!({"data":{"sessionToken":"secret","status":"CONNECTED"}});
        redact_sensitive(&mut value);
        assert_eq!(value["data"]["sessionToken"], "[REDACTED]");
        assert_eq!(value["data"]["status"], "CONNECTED");
    }

    #[test]
    fn sanitizes_sensitive_json_and_plain_text_errors() {
        let json = br#"{"error":"failed","accessToken":"secret"}"#;
        let sanitized = sanitize_text(json);
        assert!(sanitized.contains("[REDACTED]"));
        assert!(!sanitized.contains("secret"));

        let plain = b"request failed\nsession_token=secret\ntry again";
        let sanitized = sanitize_text(plain);
        assert_eq!(sanitized, "request failed\n[REDACTED]\ntry again");
    }

    #[test]
    fn test_constructor_keeps_explicit_program() {
        let cli = AgenticWalletCli::with_command(PathBuf::from("baw-test"));
        assert_eq!(cli.command, PathBuf::from("baw-test"));
    }

    #[test]
    fn wallet_commands_pin_a_stable_instance_id() {
        let command = command_for(std::path::Path::new("baw.cmd"), &["wallet", "status"]);
        let configured = command
            .as_std()
            .get_envs()
            .find(|(key, _)| *key == "BINANCE_INSTANCE_ID")
            .and_then(|(_, value)| value);
        let expected = stable_wallet_instance_id();
        assert_eq!(configured, Some(std::ffi::OsStr::new(&expected)));
    }
}
