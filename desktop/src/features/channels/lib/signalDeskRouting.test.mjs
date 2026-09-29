import assert from "node:assert/strict";
import test from "node:test";
import { routeSignalDeskRequest } from "./signalDeskRouting.ts";

test("routes a market read to Pollen", () => {
  assert.deepEqual(routeSignalDeskRequest("Analyze BTC volatility"), [
    "Pollen",
  ]);
});

test("routes account operations to Fizz", () => {
  assert.deepEqual(routeSignalDeskRequest("Cancel my order"), ["Fizz"]);
});

test("routes records to Honey", () => {
  assert.deepEqual(routeSignalDeskRequest("Find the audit history"), ["Honey"]);
});

test("routes a combined request to every relevant specialist once", () => {
  assert.deepEqual(
    routeSignalDeskRequest(
      "Analyze ETH, prepare a buy order, and record the decision",
    ),
    ["Pollen", "Fizz", "Honey"],
  );
});

test("leaves unclassified requests with Fizz as the desk lead", () => {
  assert.deepEqual(routeSignalDeskRequest("Can you help me?"), ["Fizz"]);
});

test("honors an explicit agent mention ahead of keyword routing", () => {
  assert.deepEqual(routeSignalDeskRequest("@Fizz analyze BTC"), ["Fizz"]);
});
