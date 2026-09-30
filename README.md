<h1 align="center">SignalDesk</h1>

<p align="center">
  <strong>A multi-agent desktop workspace for Binance market analysis, records, and confirmed trading actions.</strong>
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#install-on-windows">Installation</a> ·
  <a href="#connect-binance-web3-and-agentic-wallet">Binance Web3 &amp; Agentic Wallet</a> ·
  <a href="#build-from-source">Development</a> ·
  <a href="docs/SIGNALDESK_PLAN.md">Project plan</a> ·
  <a href="CONTRIBUTORS.md">Contributors</a> ·
  <a href="LICENSE">Apache 2.0</a>
</p>

## Overview

SignalDesk adapts [Block's Buzz](https://github.com/block/buzz) into a focused
Windows workspace for BSC bStock research and Agentic Wallet purchases. It
preserves Buzz's rooms, streamed agent activity, history, cancellation, and ACP
agent lifecycle while assigning the three built-in agents clear roles.

| Agent | Responsibility | Binance access |
|---|---|---|
| **Fizz** | Prepares confirmed BSC bStock purchases | Binance Web3 research and Agentic Wallet execution tools |
| **Honey** | Keeps a human-readable record of requests, decisions, approvals, order IDs, and outcomes | Read-only Binance Web3 research tools |
| **Pollen** | Retrieves bStock market data and explains trends, evidence, and uncertainty | Read-only Binance Web3 research tools |

SignalDesk 0.5.25 implements the confirmed-actions milestone. Every Fizz
purchase needs a fresh, exact confirmation from the workspace owner and
authorizes one purchase request. Honey and Pollen cannot submit trades.

## Features

- BSC bStock discovery, market data, and trade previews through Binance Web3.
- Separate trading, record-keeping, and analysis agents in one shared room.
- Streamed responses and visible tool activity.
- Cancellation through the existing stop control.
- Binance Web3 credentials stored in the operating-system credential store.
- One-action confirmation gate for Agentic Wallet bStock purchases.
- Reconciliation guidance when a write returns an uncertain result.
- Persistent conversation history and records inherited from Buzz.
- Exact-ticker BSC bStock research and read-only USDT trade previews through
  the Binance Web3 API.
- Fizz-only BSC bStock quote and execution through Binance Agentic Wallet.

## Safety model

Fizz must describe the exact action and wait for a new owner message in this
format:

```text
CONFIRM BINANCE ACTION: product=BSTOCK; action=BUY; symbol=AAPL; contract=0x...; pay_asset=USDT; pay_amount=5; type=MARKET; slippage=auto
```

The confirmation must be a single line with exact lowercase `key=value`
fields. It expires after one purchase request and does not carry into a later
turn. SignalDesk does not retry uncertain submissions automatically.

The Agentic Wallet owns and signs for the BSC wallet. SignalDesk does not
receive or store a private key or seed phrase.

## Install on Windows

### Install a packaged build

This repository does not currently publish a Windows installer. To run the
application, build it from source using the instructions below.

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

## Connect Binance Web3 and Agentic Wallet

SignalDesk uses Binance Web3 for BSC bStock research and Binance Agentic Wallet
for wallet quotes, signing, and execution:

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

- “Pollen, research SPCXB on BSC and explain its current price and market status.”
- “Fizz, prepare a 5 USDT SPCXB bStock purchase and show me the fresh quote and exact confirmation fields. Do not submit it yet.”
- “Honey, record the market analysis, decision, confirmation status, order ID,
  and final result from this conversation.”

## Architecture

```text
SignalDesk desktop (Tauri + React)
  -> Buzz relay and agent harness
    -> codex-acp
      -> local Codex CLI
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
git clone https://github.com/MainaNgureJohn/SignalDesk.git
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
trade.

The detailed implementation and acceptance status is recorded in
[`docs/SIGNALDESK_PLAN.md`](docs/SIGNALDESK_PLAN.md).

## License and attribution

SignalDesk is derived from [Block's Buzz](https://github.com/block/buzz) at
upstream revision `3c7f288c60d67df78577b237e27c3dfc8831aaa1`.

The project is licensed under the [Apache License 2.0](LICENSE). The derivative
attribution and retained component notices are documented in [NOTICE](NOTICE).
SignalDesk is a distinct project and is not presented as a Block or Binance
product.
