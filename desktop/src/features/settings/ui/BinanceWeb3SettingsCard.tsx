import * as React from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertCircle,
  CheckCircle2,
  LoaderCircle,
  ShieldCheck,
  Trash2,
  WalletCards,
} from "lucide-react";

import { invokeTauri } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { SettingsOptionGroup } from "./SettingsOptionGroup";
import { SettingsSectionHeader } from "./SettingsSectionHeader";

type Web3Status = {
  configured: boolean;
  verified: boolean;
  bscBstockMatches: number;
};

type AgenticWalletStatus = {
  installed: boolean;
  version: string | null;
  connectionStatus: string | null;
};

type AgenticWalletPairing = {
  success: boolean;
  data?: {
    status?: string;
    urlForWeb?: string;
    qrCodeId?: string;
    pairingCode?: string;
  };
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function BinanceWeb3SettingsCard() {
  const [status, setStatus] = React.useState<Web3Status | null>(null);
  const [apiKey, setApiKey] = React.useState("");
  const [secretKey, setSecretKey] = React.useState("");
  const [busy, setBusy] = React.useState<"save" | "test" | "delete" | null>(
    null,
  );
  const [error, setError] = React.useState<string | null>(null);
  const [walletStatus, setWalletStatus] =
    React.useState<AgenticWalletStatus | null>(null);
  const [balances, setBalances] = React.useState<unknown>(null);
  const [balanceBusy, setBalanceBusy] = React.useState(false);
  const [pairing, setPairing] = React.useState<
    AgenticWalletPairing["data"] | null
  >(null);
  const [walletBusy, setWalletBusy] = React.useState<
    "start" | "verify" | "signout" | null
  >(null);

  React.useEffect(() => {
    let active = true;
    void invokeTauri<Web3Status>("get_signaldesk_web3_status")
      .then((next) => {
        if (active) setStatus(next);
      })
      .catch((cause) => {
        if (active) setError(errorMessage(cause));
      });
    return () => {
      active = false;
    };
  }, []);

  const refreshWallet = React.useCallback(async () => {
    setWalletStatus(
      await invokeTauri<AgenticWalletStatus>(
        "get_signaldesk_agentic_wallet_status",
      ),
    );
  }, []);

  React.useEffect(() => {
    let active = true;
    void invokeTauri<AgenticWalletStatus>(
      "get_signaldesk_agentic_wallet_status",
    )
      .then((next) => {
        if (active) setWalletStatus(next);
      })
      .catch((cause) => {
        if (active) setError(errorMessage(cause));
      });
    return () => {
      active = false;
    };
  }, []);

  const save = async () => {
    setBusy("save");
    setError(null);
    try {
      const next = await invokeTauri<Web3Status>(
        "save_signaldesk_web3_credentials",
        { apiKey, secretKey },
      );
      setStatus(next);
      setApiKey("");
      setSecretKey("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const test = async () => {
    setBusy("test");
    setError(null);
    try {
      setStatus(
        await invokeTauri<Web3Status>("test_signaldesk_web3_connection"),
      );
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const remove = async () => {
    setBusy("delete");
    setError(null);
    try {
      await invokeTauri("delete_signaldesk_web3_credentials");
      setStatus({
        configured: false,
        verified: false,
        bscBstockMatches: 0,
      });
      setApiKey("");
      setSecretKey("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const savingDisabled =
    busy != null || apiKey.trim().length === 0 || secretKey.trim().length === 0;

  const startWalletSignIn = async () => {
    setWalletBusy("start");
    setError(null);
    try {
      const result = await invokeTauri<AgenticWalletPairing>(
        "start_signaldesk_agentic_wallet_sign_in",
      );
      if (result.data?.status === "ALREADY_CONNECTED") {
        setPairing(null);
        await refreshWallet();
      } else if (result.data?.urlForWeb && result.data.qrCodeId) {
        setPairing(result.data);
        await openUrl(result.data.urlForWeb);
      } else {
        throw new Error("Binance did not return a usable wallet pairing link.");
      }
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setWalletBusy(null);
    }
  };

  const verifyWalletSignIn = async () => {
    if (!pairing?.qrCodeId) return;
    setWalletBusy("verify");
    setError(null);
    try {
      setWalletStatus(
        await invokeTauri<AgenticWalletStatus>(
          "verify_signaldesk_agentic_wallet_sign_in",
          { qrCodeId: pairing.qrCodeId },
        ),
      );
      setPairing(null);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setWalletBusy(null);
    }
  };

  const signOutWallet = async () => {
    setWalletBusy("signout");
    setError(null);
    try {
      setWalletStatus(
        await invokeTauri<AgenticWalletStatus>(
          "sign_out_signaldesk_agentic_wallet",
        ),
      );
      setPairing(null);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setWalletBusy(null);
    }
  };

  const checkBalances = async () => {
    setBalanceBusy(true);
    setError(null);
    try {
      const result = await invokeTauri<unknown>(
        "get_signaldesk_agentic_wallet_bsc_balances",
      );
      setBalances(result);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBalanceBusy(false);
    }
  };

  return (
    <section className="space-y-6" data-testid="binance-web3-settings">
      <SettingsSectionHeader
        title="Binance Web3"
        description="Connect SignalDesk to the Binance Web3 API for BSC tokenized-stock research. Credentials stay in your operating system's protected credential store and are verified with a read-only bStock search."
      />

      {error ? (
        <div
          className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive"
          role="alert"
        >
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{error}</span>
        </div>
      ) : null}

      <SettingsOptionGroup title="Connection">
        <div className="space-y-4 p-4">
          <div className="flex items-start gap-3 rounded-lg bg-muted/30 p-3">
            {status?.configured ? (
              <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-emerald-600" />
            ) : (
              <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-muted-foreground" />
            )}
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">
                {status?.configured
                  ? status.verified
                    ? "Connected and verified"
                    : "Credentials saved"
                  : status
                    ? "Not connected"
                    : "Checking connection…"}
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                {status?.verified
                  ? `Read-only BSC test completed. AAPL returned ${status.bscBstockMatches} bStock match${status.bscBstockMatches === 1 ? "" : "es"}.`
                  : "SignalDesk never returns the saved key or secret to the interface."}
              </p>
            </div>
            {status == null && !error ? (
              <LoaderCircle className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : null}
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <label
              className="space-y-2 text-sm font-medium"
              htmlFor="binance-web3-api-key"
            >
              API key
              <Input
                autoComplete="off"
                disabled={busy != null}
                id="binance-web3-api-key"
                onChange={(event) => setApiKey(event.target.value)}
                placeholder={
                  status?.configured ? "Enter a replacement key" : "API key"
                }
                spellCheck={false}
                type="password"
                value={apiKey}
              />
            </label>
            <label
              className="space-y-2 text-sm font-medium"
              htmlFor="binance-web3-secret-key"
            >
              Secret key
              <Input
                autoComplete="off"
                disabled={busy != null}
                id="binance-web3-secret-key"
                onChange={(event) => setSecretKey(event.target.value)}
                placeholder={
                  status?.configured
                    ? "Enter a replacement secret"
                    : "Secret key"
                }
                spellCheck={false}
                type="password"
                value={secretKey}
              />
            </label>
          </div>

          <div className="flex flex-wrap gap-2">
            <Button disabled={savingDisabled} onClick={() => void save()}>
              {busy === "save" ? (
                <LoaderCircle className="h-4 w-4 animate-spin" />
              ) : null}
              Verify and save
            </Button>
            {status?.configured ? (
              <>
                <Button
                  disabled={busy != null}
                  onClick={() => void test()}
                  variant="outline"
                >
                  {busy === "test" ? (
                    <LoaderCircle className="h-4 w-4 animate-spin" />
                  ) : null}
                  Test saved connection
                </Button>
                <Button
                  disabled={busy != null}
                  onClick={() => void remove()}
                  variant="ghost"
                >
                  {busy === "delete" ? (
                    <LoaderCircle className="h-4 w-4 animate-spin" />
                  ) : (
                    <Trash2 className="h-4 w-4" />
                  )}
                  Remove credentials
                </Button>
              </>
            ) : null}
          </div>
        </div>
      </SettingsOptionGroup>

      <SettingsOptionGroup title="Agentic Wallet execution">
        <div className="space-y-4 p-4">
          <div className="flex items-start gap-3 rounded-lg bg-muted/30 p-3">
            <WalletCards className="mt-0.5 h-5 w-5 shrink-0 text-muted-foreground" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">
                {walletStatus == null
                  ? "Checking Agentic Wallet…"
                  : !walletStatus.installed
                    ? "Agentic Wallet CLI is not installed"
                    : walletStatus.connectionStatus === "CONNECTED"
                      ? "Agentic Wallet connected"
                      : "Agentic Wallet not connected"}
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                {walletStatus?.installed
                  ? `${walletStatus.version ?? "Installed"}. Fizz can quote BSC bStock buys and can submit one only after the exact owner confirmation.`
                  : "Install with: npm install -g @binance/agentic-wallet@1.10.0"}
              </p>
            </div>
            {walletStatus == null ? (
              <LoaderCircle className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : null}
          </div>

          {pairing ? (
            <div className="rounded-lg border p-3 text-sm">
              <p className="font-medium">Confirm this code in Binance App</p>
              <p className="mt-2 font-mono text-2xl tracking-[0.25em]">
                {pairing.pairingCode ?? "Check Binance App"}
              </p>
              <p className="mt-2 text-xs text-muted-foreground">
                Keep SignalDesk open while verification runs. Pairing links
                expire after about five minutes.
              </p>
            </div>
          ) : null}

          <div className="flex flex-wrap gap-2">
            {walletStatus?.installed &&
            walletStatus.connectionStatus !== "CONNECTED" ? (
              <Button
                disabled={walletBusy != null}
                onClick={() => void startWalletSignIn()}
              >
                {walletBusy === "start" ? (
                  <LoaderCircle className="h-4 w-4 animate-spin" />
                ) : null}
                Connect Agentic Wallet
              </Button>
            ) : null}
            {pairing?.qrCodeId ? (
              <Button
                disabled={walletBusy != null}
                onClick={() => void verifyWalletSignIn()}
                variant="outline"
              >
                {walletBusy === "verify" ? (
                  <LoaderCircle className="h-4 w-4 animate-spin" />
                ) : null}
                I confirmed in Binance App
              </Button>
            ) : null}
            {walletStatus?.connectionStatus === "CONNECTED" ? (
              <Button
                disabled={balanceBusy || walletBusy != null}
                onClick={() => void checkBalances()}
                variant="outline"
              >
                {balanceBusy ? (
                  <LoaderCircle className="h-4 w-4 animate-spin" />
                ) : null}
                Check BSC balances
              </Button>
            ) : null}
            {walletStatus?.connectionStatus === "CONNECTED" ? (
              <Button
                disabled={walletBusy != null}
                onClick={() => void signOutWallet()}
                variant="ghost"
              >
                {walletBusy === "signout" ? (
                  <LoaderCircle className="h-4 w-4 animate-spin" />
                ) : null}
                Disconnect wallet
              </Button>
            ) : null}
          </div>
          {balances !== null ? (
            <div className="rounded-lg border border-emerald-700/30 bg-emerald-950/20 p-3">
              <p className="mb-2 text-xs font-semibold uppercase tracking-wide">
                Live BSC balances · read only
              </p>
              <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">
                {JSON.stringify(balances, null, 2)}
              </pre>
            </div>
          ) : null}
        </div>
      </SettingsOptionGroup>
    </section>
  );
}
