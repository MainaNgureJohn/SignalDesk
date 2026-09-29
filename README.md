<h1 align="center">SignalDesk</h1>

<p align="center">
  <strong>A multi-agent desktop workspace for Binance market analysis, records, and confirmed trading actions.</strong>
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#install-on-windows">Installation</a> ·
  <a href="#connect-binance-mcp">Binance MCP</a> ·
  <a href="#build-from-source">Development</a> ·
  <a href="docs/SIGNALDESK_PLAN.md">Project plan</a> ·
  <a href="CONTRIBUTORS.md">Contributors</a> ·
  <a href="LICENSE">Apache 2.0</a>
</p>

## Overview

SignalDesk adapts [Block's Buzz](https://github.com/block/buzz) into a focused
Windows trading workspace. It preserves Buzz's rooms, streamed agent activity,
history, cancellation, and ACP agent lifecycle while assigning the three
built-in agents clear financial roles.

| Agent | Responsibility | Binance access |
|---|---|---|
| **Fizz** | Prepares confirmed Binance actions and BSC bStock purchases | Exchange tools granted by Binance OAuth; Agentic Wallet execution tools |
| **Honey** | Keeps a human-readable record of requests, decisions, approvals, order IDs, and outcomes | Read-only market and bStock research tools |
| **Pollen** | Retrieves current market data and explains trends, evidence, and uncertainty | Read-only market and bStock research tools |

SignalDesk 0.5.25 implements the confirmed-actions milestone. Every Fizz write
needs a fresh, exact confirmation from the workspace owner and authorizes one
permission request. Honey and Pollen cannot submit trades or transfers.

## Features

- Live Binance spot prices, 24-hour statistics, order books, and candlesticks.
- Separate trading, record-keeping, and analysis agents in one shared room.
- Streamed responses and visible tool activity.
- Cancellation through the existing stop control.
- Binance OAuth instead of storing API keys in the repository.
- One-action confirmation gate for trades, cancellations, conversions, and
  internal wallet transfers.
- Reconciliation guidance when a write returns an uncertain result.
- Persistent conversation history and records inherited from Buzz.
- Exact-ticker BSC bStock research and read-only USDT trade previews through
  the Binance Web3 API.
- Fizz-only BSC bStock quote and execution through Binance Agentic Wallet.

## Safety model

Fizz must describe the exact action and wait for a new owner message in this
format:

```text
CONFIRM BINANCE ACTION: product=SPOT; action=BUY; symbol=BTCUSDT; type=LIMIT; quantity=0.001; price=50000
```

Other accepted action shapes include:

```text
CONFIRM BINANCE ACTION: product=SPOT; action=CANCEL; symbol=BTCUSDT; order_id=12345
CONFIRM BINANCE ACTION: product=WALLET; action=TRANSFER; asset=USDT; amount=10; from=FUNDING; to=SPOT
CONFIRM BINANCE ACTION: product=CONVERT; action=CONVERT; from_asset=USDT; to_asset=BTC; amount=10
CONFIRM BINANCE ACTION: product=BSTOCK; action=BUY; symbol=AAPL; contract=0x...; pay_asset=USDT; pay_amount=5; type=MARKET; slippage=auto
```

The confirmation must be a single line with exact lowercase `key=value`
fields. It expires after one write request and does not carry into a later
turn. SignalDesk does not retry uncertain writes automatically.

Binance MCP does not expose external-wallet withdrawal to this application.
SignalDesk also does not fund the isolated Agentic sub-account from a main
Binance account; the account owner handles funding in Binance.

## Install on Windows

### Install a packaged build

Download the latest Windows x64 package from the
[SignalDesk releases page](https://github.com/NgureMaina/SignalDesk/releases/latest):

- [SignalDesk_0.5.25_x64-setup.exe](https://github.com/NgureMaina/SignalDesk/releases/download/v0.5.25/SignalDesk_0.5.25_x64-setup.exe) — recommended installer.
- [SignalDesk_0.5.25_x64_en-US.msi](https://github.com/NgureMaina/SignalDesk/releases/download/v0.5.25/SignalDesk_0.5.25_x64_en-US.msi) — MSI alternative.

Run one installer and follow the setup wizard. If an older SignalDesk version
is detected, select **Uninstall before installing**. Leave **Delete application
data** unchecked to preserve the existing workspace and agent records.

The current prototype packages are unsigned, so Windows may display a publisher
warning. A public production release still needs publisher code signing and a
clean-machine acceptance pass.

### Runtime requirements

SignalDesk's built-in agents use the local Codex CLI through the Codex ACP
adapter. Install these before starting the agents:

1. Install [Node.js 24 or newer](https://nodejs.org/).
2. Install and sign in to the [OpenAI Codex CLI](https://learn.chatgpt.com/docs/codex/cli).
3. Install the supported Codex ACP adapter:

```powershell
npm uninstall -g @zed-industries/codex-acp
npm install -g @agentclientprotocol/codex-acp
```

Verify the commands are available:

```powershell
codex --version
codex-acp --version
```

## Connect Binance MCP

SignalDesk uses Binance's hosted Agentic MCP endpoint. Add it to Codex with the
same server name used by the built-in personas:

```powershell
codex mcp add binance-mcp-server --url https://agent.binance.com/mcp/agentic
codex mcp login binance-mcp-server
```

Complete the Binance OAuth screen and grant the scopes needed for the workflow:

- **Market data** for all three agents.
- **Account**, **Trade**, and **Transfer** for Fizz actions.

Check the connection:

```powershell
codex mcp list
codex mcp get binance-mcp-server
```

To reconnect an existing installation:

```powershell
codex mcp logout binance-mcp-server
codex mcp login binance-mcp-server
```

The repository's [`.codex/config.toml`](.codex/config.toml) contains only the
public endpoint and the read-only tool allow-list. OAuth credentials remain in
the user's Codex credential store and must never be committed.

The command structure follows the official [Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).

## Connect Binance Web3 and Agentic Wallet

The tokenized-stock workflow uses two separate Binance connections:

- **Binance Web3 API credentials** provide bStock discovery, prices, market
  status, and read-only route previews. Add them in **Settings → Binance
  Web3**. SignalDesk stores them in the operating-system credential store.
- **Binance Agentic Wallet** owns the BSC wallet session and signs the final
  swap. SignalDesk never receives or stores a private key or seed phrase.

Install the supported Agentic Wallet CLI:

```powershell
npm install -g @binance/agentic-wallet@1.10.0
baw --version
```

Then open **Settings → Binance Web3 → Agentic Wallet execution**, select
**Connect wallet**, and complete the Binance App pairing flow. The Binance
account must have an MPC wallet. Keep enough BNB for BSC gas and enough BSC
USDT for the purchase. In the Binance App, use a small daily limit, restrict
the allowed tokens, and keep high-risk-operation confirmation enabled while
testing.

Fizz first resolves the exact bStock contract and requests a fresh Agentic
Wallet quote. It shows the contract, USDT amount, market status, slippage, and
the exact confirmation line. Only a new message containing that exact line can
authorize one submission. The returned order ID means submitted, not settled;
Fizz polls until Agentic Wallet reports `FINISHED` or `FAILED`. Honey and
Pollen do not receive the wallet status, quote, order, or execution tools.

## Start and use the agents

1. Open SignalDesk.
2. Enter the private **Market Desk** channel.
3. Start Fizz, Honey, and Pollen.
4. Ask an agent by name, or give the team a shared task.
5. For a write action, review Fizz's proposed fields and send a separate exact
   confirmation message only when they match your intent.

Example requests:

- “Pollen, check the current BTCUSDT, ETHUSDT, and BNBUSDT prices and compare
  their 24-hour performance.”
- “Pollen, analyze the BTCUSDT order book and recent candles, then explain the
  strongest support and resistance areas.”
- “Fizz, check my spot USDT balance and prepare a limit-buy plan for BTCUSDT;
  do not submit anything until I confirm the exact action.”
- “Honey, record the market analysis, decision, confirmation status, order ID,
  and final result from this conversation.”
- “Fizz, show the exact internal transfer fields for moving 10 USDT from
  Funding to Spot and wait for my confirmation.”

## Architecture

```text
SignalDesk desktop (Tauri + React)
  -> Buzz relay and agent harness
    -> codex-acp
      -> local Codex CLI
        -> Binance MCP over OAuth (exchange data and actions)
        -> SignalDesk Web3 MCP
          -> Binance Web3 API (bStock research and previews)
          -> Binance Agentic Wallet CLI (BSC quote, signing, and status)
```

The desktop and agent bridge are written in Rust and TypeScript. SignalDesk
uses Buzz's existing ACP path because it already supports agent sessions,
streaming, tool activity, cancellation, and relay-backed history.

## Build from source

### Prerequisites

- Windows 10 or 11 x64.
- [Git for Windows](https://git-scm.com/download/win).
- [Docker Desktop](https://docs.docker.com/desktop/setup/install/windows-install/).
- Microsoft C++ Build Tools with the Windows SDK.
- Microsoft Edge WebView2 Runtime.
- Node.js 24+, pnpm 10+, Rust, and `just`; the repository's
  [Hermit](https://cashapp.github.io/hermit/) environment supplies pinned
  versions of the development tools.
- Codex CLI and `@agentclientprotocol/codex-acp` as described above.

Run the setup from Git Bash:

```bash
git clone https://github.com/NgureMaina/SignalDesk.git
cd SignalDesk
. ./bin/activate-hermit
just setup
just dev
```

`just setup` installs dependencies, creates the local `.env` file, starts the
Docker services, and runs migrations. `just dev` starts the relay and desktop
app. The local relay is available at `ws://localhost:3000`.

Useful development checks:

```bash
just desktop-typecheck
just desktop-tauri-test
cargo test -p buzz-acp
just ci
```

Build an unsigned Windows package with the `x86_64-pc-windows-msvc` Rust target
after the required sidecar binaries have been built and placed in
`desktop/src-tauri/binaries`:

```bash
cd desktop
pnpm tauri build --target x86_64-pc-windows-msvc
```

Tauri writes the installer bundles under
`desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/`.

## Validation status

The 0.5.25 milestone passed the focused SignalDesk Rust tests, persona migration
tests, runtime selection test, TypeScript typecheck, Rust formatting, release
build, and Windows installer inspection. No automated test performs a live
trade, cancellation, conversion, or transfer.

The detailed implementation and acceptance status is recorded in
[`docs/SIGNALDESK_PLAN.md`](docs/SIGNALDESK_PLAN.md).

## License and attribution

SignalDesk is derived from [Block's Buzz](https://github.com/block/buzz) at
upstream revision `3c7f288c60d67df78577b237e27c3dfc8831aaa1`.

The project is licensed under the [Apache License 2.0](LICENSE). The derivative
attribution and retained component notices are documented in [NOTICE](NOTICE).
SignalDesk is a distinct project and is not presented as a Block or Binance
product.
