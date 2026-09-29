export type SignalDeskAgentName = "Pollen" | "Fizz" | "Honey";

const MARKET =
  /\b(market|price|chart|trend|technical|fundamental|volatility|volume|btc|eth|bnb|bitcoin|ethereum|forecast|research|analy[sz]e|analysis|signal|risk)\b/i;
const OPERATION =
  /\b(trade|buy|sell|order|position|portfolio|balance|convert|conversion|transfer|cancel|execute|place|rebalance|account|deposit|withdraw)\b/i;
const RECORDS =
  /\b(record|log|journal|history|audit|receipt|report|document|summari[sz]e|summary|decision|what did we|previous|past)\b/i;

/** Selects personas for a request; authorization remains with each agent/tool. */
export function routeSignalDeskRequest(content: string): SignalDeskAgentName[] {
  const explicit = (["Pollen", "Fizz", "Honey"] as const).filter((name) =>
    new RegExp(`@${name}\\b`, "i").test(content),
  );
  if (explicit.length > 0) return [...explicit];
  const names: SignalDeskAgentName[] = [];
  if (MARKET.test(content)) names.push("Pollen");
  if (OPERATION.test(content)) names.push("Fizz");
  if (RECORDS.test(content)) names.push("Honey");
  return names.length > 0 ? names : ["Fizz"];
}
