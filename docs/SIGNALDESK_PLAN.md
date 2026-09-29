# SignalDesk: workflow and milestone plan

SignalDesk is a Windows desktop workspace derived from Block's Buzz. It keeps
Buzz's conversations, agent lifecycle, teams, relay-backed history, streamed
activity, and cancellation. The first team has three jobs:

- **Fizz — Trader:** prepares and, when the user has granted the required
  Binance scopes, submits one action after a fresh exact confirmation.
- **Honey — Records:** records requests, tool results, decisions, approvals,
  order identifiers, and outcomes in the shared workspace. Honey never submits
  orders or transfers.
- **Pollen — Market Analyst:** retrieves current Binance market data, compares
  BTC, ETH, and BNB, and explains evidence and uncertainty. Pollen never submits
  orders or transfers.

## User workflow

1. The user writes a request in the private team channel.
2. The relevant agent calls Binance MCP and Buzz shows the tool activity.
3. Codex streams the answer into the conversation. The user can cancel the
   active turn using Buzz's existing stop control.
4. Before a write, Fizz restates the exact action and waits. The owner replies
   in a new message with `CONFIRM BINANCE ACTION:` followed by exact `key=value`
   fields. That turn can approve only one permission request.
5. Honey writes a durable, human-readable record in the workspace. External
   withdrawals are not offered by the Binance MCP server.

## Requirements

- Windows desktop app with distinct SignalDesk branding.
- Preserve Buzz agent/team behavior; use its three built-in personas.
- Use the local Codex installation through Buzz's existing Codex ACP adapter.
- Configure Binance at `https://agent.binance.com/mcp/agentic`.
- Show streamed answers, tool activity, errors, and cancellation honestly.
- Use live Binance data in the acceptance workflow; never simulate a successful
  Binance or Codex connection.
- Keep configuration inside this checkout. Do not read ORBIT credentials or
  modify the user's global Codex configuration.
- Keep Honey and Pollen restricted to spot price, 24-hour ticker, order-book,
  and candlestick tools.
- Give only Fizz access to tools authorized by the user's Binance account,
  trade, and transfer scopes, with write approvals enforced by the ACP bridge.

## Architecture

```text
SignalDesk desktop (Buzz Tauri + React)
  -> Buzz relay and agent harness (existing conversation/history path)
    -> codex-acp (existing Buzz Codex adapter)
      -> local Codex CLI
        -> Binance MCP (centralized-market reads and confirmed actions)
        -> SignalDesk Web3 MCP sidecar
          -> Binance Web3 REST API (BSC RWA research and trade previews)
          -> Binance Agentic Wallet CLI (BSC quotes, signing, and order status)
```

Buzz's ACP integration is the cheaper adaptation today. It already maps Buzz
messages to agent sessions, emits tool activity, streams updates, and handles
cancellation. Replacing it with Codex App Server would remove the third-party
ACP adapter and expose Codex threads, approvals, streamed item events, and
`turn/interrupt` directly, but it would require a new Rust protocol client and
translation layer across Buzz's relay/session model. That is a later hardening
step, not a prerequisite for the first useful app.

## Milestones

1. **Live read-only slice:** distinct app identity and welcome copy; three
   financial roles; isolated Binance MCP config; real BTC/ETH/BNB retrieval;
   streamed answer, visible tool activity, and cancellation.
2. **Record workflow:** structured Honey ledger entries correlated with the
   originating request and Binance tool result; export and reconciliation.
3. **Account connection:** Binance OAuth onboarding, eligibility/region errors,
   isolated Agentic sub-account, and explicit scope display. No funding step is
   automated.
4. **Confirmed actions:** Fizz-only trading and internal-wallet-transfer tools,
   confirmation before every write, idempotency, reconciliation, and emergency
   stop guidance.
5. **App Server evaluation:** replace ACP only if direct Codex thread and
   approval control materially reduces complexity after milestones 1–4.

## Tokenized Stocks Edition extension

SignalDesk's next workflow targets tokenized stocks on BNB Smart Chain. It uses
the Binance Web3 REST API rather than treating the existing Binance Agentic MCP
as an onchain trading interface. The first focused product slice is a bStock
research and risk packet: resolve a ticker to its BSC contract, compare onchain
and underlying prices, report the reference-market status, then prepare and
simulate a USDT trade before asking the owner for exact approval.

The shared `signaldesk-web3` crate implements the authenticated read-only
foundation. It signs the exact wire path with the required `/build` prefix,
keeps decimal values as strings, limits response size, and exposes bStock/Ondo
search, BSC token listing, price lookup, and underlying-market status. The
desktop settings screen verifies API credentials with a read-only AAPL bStock
search, stores them in a dedicated operating-system credential entry, and can
retest or remove them without returning saved secrets to the UI.
The desktop also exposes one exact-ticker research operation that resolves a
single BSC bStock contract and fetches its token price, reference price, and
underlying-market status as one packet. Fuzzy or ambiguous ticker matches stop
with an error instead of selecting a contract silently.

Stored credentials are not placed in conversation events, Codex prompts, or
the Codex process environment. They enter React state only while the owner is
typing them and are cleared after a successful save. A bundled read-only MCP
sidecar loads the dedicated credential entry and exposes the exact-ticker
research packet to Fizz, Honey, and Pollen. It also produces a read-only BSC
USDT buy preview for an exact ticker and receiving wallet. The preview resolves
the bStock contract, converts the requested USDT value without floating-point
math, and returns Binance Web3's best short-lived RFQ route. It does not build
calldata or imply that a trade occurred. Existing unmodified built-in agent
records are upgraded in place; customized agent configuration is preserved.
The current source adds a bounded adapter around Binance Agentic Wallet. The
wallet CLI owns authentication and signing; SignalDesk does not store a private
key, seed phrase, or wallet session token. Fizz can inspect wallet status and
BSC balances, request a BSC USDT-to-bStock quote, submit one market swap after
the owner sends the quote's exact confirmation line, and poll the returned
order identifier to `FINISHED` or `FAILED`. Submission is reported as pending,
never as final. The ACP permission gate binds the wallet tool's confirmation
argument to the exact current owner message and consumes it after one attempt.
The adapter fixes the chain to BSC (`56`) and the payment token
to BSC USDT, validates all dynamic command arguments, caps subprocess output,
sets deadlines, and redacts credential-shaped fields before returning results.

Honey and Pollen receive an explicit allow-list containing only
`research_bsc_bstock` and `preview_bsc_bstock_buy`; they cannot access wallet
status, balances, quotes, orders, or execution. Existing exact SignalDesk
persona configurations migrate to these permissions while customized records
remain unchanged. Automated tests do not connect a wallet or submit a live
trade. Live acceptance still requires a deliberately funded MPC wallet, small
Binance App limits, BNB for gas, BSC USDT, and a separate owner confirmation.

## Current build and connection status

Buzz requires Node 24+, pnpm 10+, Rust, the Tauri 2 Windows prerequisites
(Microsoft C++ Build Tools and WebView2), plus its bundled Rust sidecars. Full
relay development also uses Docker for PostgreSQL, Redis, MinIO, Keycloak, and
Prometheus. This machine has Node 24, pnpm, Codex 0.153.4, `codex-acp` 1.1.7,
Microsoft C++ Build Tools with the Windows 11 SDK, WebView2 Runtime 152, and a
workspace-local Rust 1.95.0 MSVC toolchain. The required Windows sidecars were
built and bundled successfully.

The authenticated `binance-mcp-server` OAuth connection is available to the
normal Codex CLI. A live end-to-end probe through Codex and Binance MCP returned
BTCUSDT, ETHUSDT, and BNBUSDT prices. Binance's MCP endpoint requires OAuth even
for these public tools; public means no Binance account-data scope is needed,
not that MCP transport authentication is skipped.

SignalDesk 0.5.24 completed the read-only slice and repaired stale built-in
harness references created by the 0.5.23 prototype. The confirmed-actions
milestone is packaged in 0.5.25: Fizz requests write permissions, the ACP bridge
rejects unconfirmed writes, a fresh exact owner confirmation arms one request,
and the decision is emitted to observer telemetry. Honey and Pollen remain
read-only. The live acceptance pass still requires reconnecting Binance with
account, trade, and transfer scopes. A distributable public release also needs
publisher code signing and a clean-machine acceptance pass.

The task-local Binance configuration is seeded into the built-in personas
through the documented `CODEX_CONFIG` bridge. All three personas select the
canonical `codex` runtime, which resolves to the `codex-acp` adapter command,
and the server name matches the authenticated `binance-mcp-server` entry. Fizz
uses Binance's scope-authorized catalog with write approvals; Honey and Pollen
retain the four-tool read-only allow-list. No global Codex setting is changed.

## Upstream and licensing record

- Upstream: `https://github.com/block/buzz`
- Revision: `3c7f288c60d67df78577b237e27c3dfc8831aaa1`
- Revision date: 2026-09-05
- Upstream license: Apache License 2.0
- Upstream has no repository-level NOTICE file. Component notices retained in
  this checkout include the proof-of-work license, Inter font license, and
  Pocket Voices notice.
- SignalDesk preserves the upstream LICENSE and adds a derivative NOTICE. It
  uses a separate product name and Windows application identifier.
