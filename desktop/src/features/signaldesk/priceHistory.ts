export type PriceObservation = { at: number; price: number };
export type PriceHistory = Record<string, PriceObservation[]>;

export type SavedTracking = {
  tickers: string[];
  history: PriceHistory;
};

export const DEFAULT_BSTOCK_TICKERS = ["NVDA", "AAPL", "MSFT", "TSLA"];
export const TRACKING_WINDOW_MS = 24 * 60 * 60 * 1_000;
const MAX_TICKERS = 12;
const MAX_OBSERVATIONS = 96;
const FUTURE_TOLERANCE_MS = 5 * 60 * 1_000;
const STORAGE_PREFIX = "signaldesk.bstock.tracking.v1";

export function trackingStorageKey(pubkey: string) {
  return `${STORAGE_PREFIX}:${pubkey.toLowerCase()}`;
}

export function cleanTicker(value: string) {
  const ticker = value.trim().toUpperCase();
  return /^[A-Z]{1,6}$/.test(ticker) ? ticker : null;
}

export function emptyTracking(): SavedTracking {
  return { tickers: [...DEFAULT_BSTOCK_TICKERS], history: {} };
}

export function readTracking(
  storage: Storage,
  key: string,
  now = Date.now(),
): SavedTracking {
  try {
    const raw = storage.getItem(key);
    if (!raw) return emptyTracking();
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return emptyTracking();
    const saved = parsed as { tickers?: unknown; history?: unknown };
    const tickers = Array.isArray(saved.tickers)
      ? [
          ...new Set(
            saved.tickers
              .filter((value): value is string => typeof value === "string")
              .map(cleanTicker)
              .filter((value): value is string => value !== null),
          ),
        ].slice(0, MAX_TICKERS)
      : [...DEFAULT_BSTOCK_TICKERS];
    const history: PriceHistory = {};
    if (saved.history && typeof saved.history === "object") {
      for (const ticker of tickers) {
        const entries = (saved.history as Record<string, unknown>)[ticker];
        if (!Array.isArray(entries)) continue;
        history[ticker] = entries
          .filter((entry): entry is PriceObservation =>
            Boolean(
              entry &&
                typeof entry === "object" &&
                Number.isFinite(entry.at) &&
                Number.isFinite(entry.price) &&
                entry.price > 0 &&
                entry.at >= now - TRACKING_WINDOW_MS &&
                entry.at <= now + FUTURE_TOLERANCE_MS,
            ),
          )
          .sort((a, b) => a.at - b.at)
          .slice(-MAX_OBSERVATIONS);
      }
    }
    return {
      tickers: tickers.length ? tickers : [...DEFAULT_BSTOCK_TICKERS],
      history,
    };
  } catch {
    return emptyTracking();
  }
}

export function writeTracking(
  storage: Storage,
  key: string,
  tracking: SavedTracking,
) {
  try {
    storage.setItem(key, JSON.stringify(tracking));
    return true;
  } catch {
    return false;
  }
}

export function addObservation(
  history: PriceHistory,
  ticker: string,
  price: number,
  at: number,
  now = Date.now(),
): PriceHistory {
  const cutoff = now - TRACKING_WINDOW_MS;
  const current = (history[ticker] ?? []).filter((entry) => entry.at >= cutoff);
  if (
    !Number.isFinite(price) ||
    price <= 0 ||
    !Number.isFinite(at) ||
    at < cutoff ||
    at > now + FUTURE_TOLERANCE_MS ||
    (current.length > 0 && at <= (current[current.length - 1]?.at ?? 0))
  ) {
    return current.length === (history[ticker]?.length ?? 0)
      ? history
      : { ...history, [ticker]: current };
  }
  return {
    ...history,
    [ticker]: [...current, { at, price }].slice(-MAX_OBSERVATIONS),
  };
}

export function trackedMove(observations: PriceObservation[] | undefined) {
  if (!observations || observations.length < 2) return null;
  const first = observations[0];
  const last = observations[observations.length - 1];
  if (!first || !last || first.at >= last.at || first.price <= 0) return null;
  return {
    percent: (last.price / first.price - 1) * 100,
    from: first.at,
    to: last.at,
  };
}
