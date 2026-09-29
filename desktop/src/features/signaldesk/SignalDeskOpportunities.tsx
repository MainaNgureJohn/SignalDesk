import * as React from "react";
import { ArrowRight, Plus, RefreshCw, TrendingUp, X } from "lucide-react";
import { invokeTauri } from "@/shared/api/tauri";
import {
  addObservation,
  cleanTicker,
  emptyTracking,
  readTracking,
  trackedMove,
  trackingStorageKey,
  writeTracking,
} from "./priceHistory";
import "./signalDeskOpportunities.css";

type Research = {
  ticker: string;
  companyName: string;
  asset: { tokenSymbol: string; tokenContractAddress: string };
  price: {
    tokenPrice: string;
    referencePrice: string;
    tokenPriceUpdatedAt: number;
  };
  underlyingMarket: {
    statusInfo: { openState: boolean; marketStatus: string };
    marketData: {
      high52W?: string | null;
      low52W?: string | null;
      volumeShares24H?: string | null;
    };
  } | null;
  underlyingMarketError?: string | null;
  marketStatus?: { openState: boolean; marketStatus: string } | null;
  marketStatusError?: string | null;
};

const REFRESH_MS = 60_000;
const MAX_TICKERS = 12;

function numeric(value: string | null | undefined) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function money(value: string | null | undefined) {
  const amount = numeric(value);
  return amount === null
    ? "—"
    : new Intl.NumberFormat(undefined, {
        style: "currency",
        currency: "USD",
        maximumFractionDigits: amount < 1 ? 4 : 2,
      }).format(amount);
}

function analysis(packet: Research) {
  const token = numeric(packet.price.tokenPrice);
  const reference = numeric(packet.price.referencePrice);
  if (token === null || reference === null)
    return "Price comparison unavailable.";
  const gap = ((token / reference - 1) * 100).toFixed(1);
  const direction = token >= reference ? "above" : "below";
  const status = packet.marketStatus ?? packet.underlyingMarket?.statusInfo;
  const session = !status
    ? "The reference market session is unavailable."
    : status.openState
      ? "The reference market is open."
      : "The reference market is closed; its price may lag.";
  return `The bStock token is ${Math.abs(Number(gap))}% ${direction} the underlying reference. ${session}`;
}

export function SignalDeskOpportunities({
  identityPubkey,
  onOpenConnection,
}: {
  identityPubkey?: string;
  onOpenConnection: () => void;
}) {
  const storageKey = identityPubkey ? trackingStorageKey(identityPubkey) : null;
  const [tracking, setTracking] = React.useState(() => {
    if (!storageKey) return emptyTracking();
    try {
      return readTracking(window.localStorage, storageKey);
    } catch {
      return emptyTracking();
    }
  });
  const tickers = tracking.tickers;
  const [input, setInput] = React.useState("");
  const [research, setResearch] = React.useState<Record<string, Research>>({});
  const [selected, setSelected] = React.useState(() => tickers[0] ?? "NVDA");
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [lastChecked, setLastChecked] = React.useState<number | null>(null);
  const [historySaved, setHistorySaved] = React.useState(true);
  const refreshVersion = React.useRef(0);

  React.useEffect(() => {
    if (!storageKey) return;
    try {
      setHistorySaved(writeTracking(window.localStorage, storageKey, tracking));
    } catch {
      setHistorySaved(false);
    }
  }, [storageKey, tracking]);

  const refresh = React.useCallback(async () => {
    const version = ++refreshVersion.current;
    setLoading(true);
    const results: PromiseSettledResult<Research>[] = [];
    for (const ticker of tickers) {
      try {
        results.push({
          status: "fulfilled",
          value: await invokeTauri<Research>("research_signaldesk_bstock", {
            ticker,
          }),
        });
      } catch (reason) {
        results.push({ status: "rejected", reason });
      }
    }
    if (version !== refreshVersion.current) return;
    const next: Record<string, Research> = {};
    const failures: string[] = [];
    const reasons: string[] = [];
    results.forEach((result, index) => {
      const ticker = tickers[index];
      if (!ticker) return;
      if (result.status === "fulfilled") next[ticker] = result.value;
      else {
        failures.push(ticker);
        reasons.push(
          result.reason instanceof Error
            ? result.reason.message
            : String(result.reason),
        );
      }
    });
    setResearch((previous) => {
      const updated = { ...previous };
      Object.entries(next).forEach(([ticker, packet]) => {
        if (
          !updated[ticker] ||
          packet.price.tokenPriceUpdatedAt >=
            updated[ticker].price.tokenPriceUpdatedAt
        ) {
          updated[ticker] = packet;
        }
      });
      return updated;
    });
    setTracking((previous) => {
      let history = previous.history;
      const now = Date.now();
      Object.entries(next).forEach(([ticker, packet]) => {
        if (!previous.tickers.includes(ticker)) return;
        const price = numeric(packet.price.tokenPrice);
        if (price !== null) {
          history = addObservation(
            history,
            ticker,
            price,
            packet.price.tokenPriceUpdatedAt,
            now,
          );
        }
      });
      return history === previous.history ? previous : { ...previous, history };
    });
    setError(
      failures.length
        ? `Live research is unavailable for ${failures.join(", ")}. ${reasons[0]?.slice(0, 300) ?? "Try again."}`
        : null,
    );
    setLastChecked(Date.now());
    setLoading(false);
  }, [tickers]);

  React.useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), REFRESH_MS);
    return () => {
      window.clearInterval(timer);
      refreshVersion.current += 1;
    };
  }, [refresh]);

  const rows = React.useMemo(
    () =>
      tickers
        .map((ticker) => {
          const packet = research[ticker];
          const move = trackedMove(tracking.history[ticker]);
          return { ticker, packet, move };
        })
        .sort(
          (a, b) =>
            (b.move?.percent ?? -Infinity) - (a.move?.percent ?? -Infinity),
        ),
    [research, tracking.history, tickers],
  );
  const selectedPacket = research[selected];
  const selectedRow = rows.find((row) => row.ticker === selected);

  const addTicker = (event: React.FormEvent) => {
    event.preventDefault();
    const ticker = cleanTicker(input);
    if (!ticker || tickers.includes(ticker) || tickers.length >= MAX_TICKERS)
      return;
    setTracking((current) => ({
      ...current,
      tickers: [...current.tickers, ticker],
    }));
    setSelected(ticker);
    setInput("");
  };

  const removeSelected = () => {
    if (tickers.length <= 1) return;
    const remaining = tickers.filter((ticker) => ticker !== selected);
    setTracking((current) => ({
      tickers: remaining,
      history: Object.fromEntries(
        Object.entries(current.history).filter(
          ([ticker]) => ticker !== selected,
        ),
      ),
    }));
    setResearch((current) =>
      Object.fromEntries(
        Object.entries(current).filter(([ticker]) => ticker !== selected),
      ),
    );
    setSelected(remaining[0] ?? "");
  };

  return (
    <div className="sd-opportunities" data-testid="signaldesk-opportunities">
      <section className="sd-opportunities-hero">
        <div className="sd-overline">◇ BNB CHAIN / TOKENIZED EQUITIES</div>
        <h1>
          Market <em>opportunities.</em>
        </h1>
        <p>
          Follow BNB Chain bStocks, compare each token with its underlying
          stock, and watch how your tracked symbols move over time.
        </p>
        <div className="sd-opportunities-note">
          <span>READ ONLY</span>
          <span>BINANCE WEB3 SOURCE</span>
          <span>REFRESHES EVERY 60 SECONDS</span>
        </div>
      </section>

      <div className="sd-opportunities-grid">
        <section className="sd-watch-panel">
          <div className="sd-opportunities-panel-head">
            <div>
              <small>01 / YOUR MARKET RADAR</small>
              <h2>Tracked bStocks</h2>
            </div>
            <button
              aria-label="Refresh prices"
              disabled={loading}
              onClick={() => void refresh()}
              type="button"
            >
              <RefreshCw size={16} className={loading ? "sd-spin" : ""} />
            </button>
          </div>
          <div className="sd-watch-caption">
            Ranked by observed price change over each symbol’s saved window (up
            to 24 hours). Two distinct updates are required; this feed does not
            provide a market-wide ranking.
          </div>
          <div className="sd-watch-head">
            <span>ASSET</span>
            <span>TOKEN PRICE</span>
            <span>TRACKED MOVE</span>
          </div>
          <div className="sd-watch-list">
            {rows.map(({ ticker, packet, move }, index) => (
              <button
                aria-pressed={selected === ticker}
                className={`sd-watch-row ${selected === ticker ? "is-active" : ""}`}
                key={ticker}
                onClick={() => setSelected(ticker)}
                type="button"
              >
                <span className="sd-watch-asset">
                  <b>{String(index + 1).padStart(2, "0")}</b>
                  <span className="sd-watch-icon">{ticker.slice(0, 1)}</span>
                  <span>
                    <strong>{ticker}</strong>
                    <small>
                      {packet?.asset.tokenSymbol ?? "BNB CHAIN BSTOCK"}
                    </small>
                  </span>
                </span>
                <strong>{money(packet?.price.tokenPrice)}</strong>
                <span
                  className={`sd-watch-change ${move && move.percent < 0 ? "is-down" : ""}`}
                >
                  {move === null
                    ? "—"
                    : `${move.percent >= 0 ? "+" : ""}${move.percent.toFixed(2)}%`}
                </span>
              </button>
            ))}
          </div>
          <form className="sd-add-ticker" onSubmit={addTicker}>
            <input
              aria-label="Add stock ticker"
              maxLength={6}
              onChange={(event) => setInput(event.target.value)}
              placeholder="Add exact ticker, e.g. AMZN"
              value={input}
            />
            <button
              type="submit"
              aria-label="Track ticker"
              disabled={tickers.length >= MAX_TICKERS}
            >
              <Plus size={17} /> Track
            </button>
          </form>
          <div className="sd-watch-tools">
            <span>
              {tickers.length} / {MAX_TICKERS} tracked
            </span>
            {tickers.length > 1 ? (
              <button onClick={removeSelected} type="button">
                <X size={13} /> Remove {selected}
              </button>
            ) : null}
          </div>
          {error ? (
            <div className="sd-feed-error" role="status">
              {error}
              <button onClick={onOpenConnection} type="button">
                Open connection settings <ArrowRight size={13} />
              </button>
            </div>
          ) : null}
          <div className="sd-feed-time">
            {lastChecked
              ? `Last checked ${new Date(lastChecked).toLocaleTimeString()}`
              : "Waiting for first update"}
            {!historySaved ? " · History cannot be saved on this device" : ""}
          </div>
        </section>

        <section className="sd-insight-panel">
          <div className="sd-opportunities-panel-head">
            <div>
              <small>02 / SIMPLE MARKET READ</small>
              <h2>{selected} at a glance</h2>
            </div>
            <span className="sd-chain-chip">◇ BNB CHAIN</span>
          </div>
          <div className="sd-insight-price">
            <span>TOKEN PRICE</span>
            <strong>{money(selectedPacket?.price.tokenPrice)}</strong>
            <small>
              {selectedPacket
                ? selectedPacket.companyName
                : "Connect Binance Web3 to load a live read"}
            </small>
          </div>
          <div className="sd-insight-metrics">
            <div>
              <span>UNDERLYING STOCK</span>
              <strong>{money(selectedPacket?.price.referencePrice)}</strong>
            </div>
            <div>
              <span>TRACKED MOVE</span>
              <strong>
                {selectedRow?.move === null || selectedRow?.move === undefined
                  ? "—"
                  : `${selectedRow.move.percent >= 0 ? "+" : ""}${selectedRow.move.percent.toFixed(2)}%`}
              </strong>
              {selectedRow?.move ? (
                <small>
                  From {new Date(selectedRow.move.from).toLocaleString()}
                </small>
              ) : null}
            </div>
            <div>
              <span>REFERENCE MARKET</span>
              <strong>
                {selectedPacket?.marketStatus?.marketStatus ??
                  selectedPacket?.underlyingMarket?.statusInfo.marketStatus ??
                  "—"}
              </strong>
            </div>
            <div>
              <span>52 WEEK RANGE</span>
              <strong>
                {selectedPacket?.underlyingMarket
                  ? `${money(selectedPacket.underlyingMarket.marketData.low52W)} – ${money(selectedPacket.underlyingMarket.marketData.high52W)}`
                  : "—"}
              </strong>
            </div>
          </div>
          <div className="sd-insight-analysis">
            <div>
              <TrendingUp size={18} /> SIGNALDESK READ
            </div>
            <p>
              {selectedPacket
                ? analysis(selectedPacket)
                : "Live token and reference prices will appear here after the Binance Web3 connection is ready. Add a ticker to track another bStock."}
            </p>
            <small>
              {selectedPacket
                ? `Price updated ${new Date(selectedPacket.price.tokenPriceUpdatedAt).toLocaleString()}. This is a price comparison, not a trade recommendation.`
                : "No live price is shown until the feed responds."}
            </small>
          </div>
        </section>
      </div>
    </div>
  );
}
