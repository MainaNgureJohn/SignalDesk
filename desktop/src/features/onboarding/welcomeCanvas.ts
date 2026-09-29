import { getCanvas, setCanvas } from "@/shared/api/tauri";

export const WELCOME_CANVAS_CONTENT = `# Welcome to SignalDesk

This private channel is your home base for market work. Fizz prepares confirmed Binance actions, Honey keeps the audit record, and Pollen provides market analysis and insights.

## Work with your agents

- Mention an agent when you want its help.
- Bring multiple agents into the same conversation when you want different perspectives.
- Keep decisions, progress, and results in the channel so everyone shares the same context.

## Quick challenge

Ask Pollen for current BTC, ETH, and BNB market status, ask Fizz to turn the analysis into an action plan, and ask Honey to record the decision.

## Get help

Ask the team a question here. The [Buzz user guide](https://github.com/block/buzz) also documents the upstream collaboration features. SignalDesk is derived from Block's Buzz under Apache-2.0.
`;

type WelcomeCanvasClient = {
  getCanvas: typeof getCanvas;
  setCanvas: typeof setCanvas;
};

/** Seed the Welcome canvas without overwriting anything the user has written. */
export async function ensureWelcomeCanvas(
  channelId: string,
  client: WelcomeCanvasClient = { getCanvas, setCanvas },
) {
  const existing = await client.getCanvas(channelId);
  // Nullish (not `!== null`) so an absent field can never masquerade as an
  // existing canvas — that exact mismatch silently skipped seeding before.
  if (existing.updatedAt != null || existing.author != null) {
    return false;
  }

  await client.setCanvas({ channelId, content: WELCOME_CANVAS_CONTENT });
  return true;
}
