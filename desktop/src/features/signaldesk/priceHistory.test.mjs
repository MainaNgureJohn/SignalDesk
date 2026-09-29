import assert from "node:assert/strict";
import test from "node:test";
import {
  addObservation,
  emptyTracking,
  readTracking,
  trackedMove,
  trackingStorageKey,
  writeTracking,
} from "./priceHistory.ts";

const NOW = 1_800_000_000_000;

function memoryStorage() {
  const values = new Map();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
}

test("needs two distinct source updates before ranking a ticker", () => {
  const first = addObservation({}, "NVDA", 100, NOW - 60_000, NOW);
  assert.equal(trackedMove(first.NVDA), null);
  const duplicate = addObservation(first, "NVDA", 105, NOW - 60_000, NOW);
  assert.equal(duplicate, first);
  const second = addObservation(first, "NVDA", 110, NOW, NOW);
  const move = trackedMove(second.NVDA);
  assert.equal(move?.percent.toFixed(2), "10.00");
  assert.equal(move?.from, NOW - 60_000);
  assert.equal(move?.to, NOW);
});

test("rejects stale, invalid, and future prices", () => {
  const original = addObservation({}, "AAPL", 200, NOW - 1_000, NOW);
  assert.equal(
    addObservation(original, "AAPL", 300, NOW - 2_000, NOW),
    original,
  );
  assert.equal(addObservation(original, "AAPL", 0, NOW, NOW), original);
  assert.equal(
    addObservation(original, "AAPL", 300, NOW + 10 * 60_000, NOW),
    original,
  );
});

test("persists only bounded, valid recent observations per identity", () => {
  const storage = memoryStorage();
  const key = trackingStorageKey("ABCDEF");
  assert.equal(key, trackingStorageKey("abcdef"));
  assert.equal(
    writeTracking(storage, key, {
      tickers: ["NVDA", "AAPL"],
      history: {
        NVDA: [
          { at: NOW - 25 * 60 * 60_000, price: 90 },
          { at: NOW - 60_000, price: 100 },
          { at: NOW, price: 110 },
        ],
      },
    }),
    true,
  );
  const loaded = readTracking(storage, key, NOW);
  assert.deepEqual(loaded.tickers, ["NVDA", "AAPL"]);
  assert.deepEqual(loaded.history.NVDA, [
    { at: NOW - 60_000, price: 100 },
    { at: NOW, price: 110 },
  ]);
  assert.equal(trackedMove(loaded.history.NVDA)?.percent.toFixed(2), "10.00");
  storage.setItem(key, "{broken");
  assert.deepEqual(readTracking(storage, key, NOW), emptyTracking());
});
