import { expect, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";

type MockAgent = { name: string; pubkey: string; status: string };

test("starts a new desk without the Buzz identity sign-in flow", async ({
  page,
}) => {
  await installMockBridge(
    page,
    { identityStorage: "ephemeral" },
    {
      skipOnboardingSeed: true,
      skipCommunitySeed: true,
    },
  );
  await page.goto("/");
  await expect
    .poll(async () =>
      page.evaluate(
        () =>
          window.__BUZZ_E2E_COMMANDS__?.includes("persist_current_identity") ??
          false,
      ),
    )
    .toBe(true);
  await expect(
    page.getByRole("heading", { name: "Enter your private key" }),
  ).toHaveCount(0);
  await expect(page.getByTestId("machine-onboarding-gate")).toHaveCount(0);
});

test("opens agent runtime settings from the OpenClaw card", async ({
  page,
}) => {
  await installMockBridge(page);
  await page.goto("/");

  const workspace = page.getByTestId("signaldesk-workspace");
  const openClaw = workspace.locator(".sd-openclaw-slot");
  await expect(openClaw).toBeVisible();
  await expect(openClaw.locator(".sd-openclaw-status")).toContainText(
    /RUNTIME|SETUP|STATUS/,
  );
  await openClaw.getByRole("button", { name: "Open agent settings" }).click();
  await expect(page.getByTestId("settings-agents")).toBeVisible();
  await expect(page.getByTestId("settings-harnesses")).toBeVisible();
});

test("routes a desk request and keeps each agent reply visible", async ({
  page,
}) => {
  await installMockBridge(page);
  await page.goto("/");

  const workspace = page.getByTestId("signaldesk-workspace");
  await expect(workspace).toBeVisible();
  await expect(workspace.getByText("PRIVATE DESK LIVE")).toBeVisible();
  await expect
    .poll(async () => {
      const agents = await page.evaluate(async () => {
        const invoke = (
          window as Window & {
            __TAURI_INTERNALS__?: {
              invoke: (command: string) => Promise<MockAgent[]>;
            };
          }
        ).__TAURI_INTERNALS__?.invoke;
        return invoke ? await invoke("list_managed_agents") : [];
      });
      return ["Pollen", "Honey"].every((name) =>
        agents.some(
          (agent) =>
            agent.name === name &&
            (agent.status === "running" || agent.status === "deployed"),
        ),
      );
    })
    .toBe(true);

  const newTaskButton = workspace.getByRole("button", { name: "+ New task" });
  if (await newTaskButton.count()) await newTaskButton.click();
  await workspace
    .getByRole("textbox", { name: "NEW TASK" })
    .fill("Analyze BTC market conditions and record a concise summary");
  await workspace.getByRole("button", { name: "Start task" }).click();
  await expect(workspace.getByText("REQUEST RECEIVED")).toBeVisible();
  await expect(workspace.getByText("Routed")).toHaveCount(2);

  const team = await page.evaluate(async () => {
    const invoke = (
      window as Window & {
        __TAURI_INTERNALS__?: {
          invoke: (command: string) => Promise<unknown>;
        };
      }
    ).__TAURI_INTERNALS__?.invoke;
    if (!invoke) throw new Error("Mock Tauri bridge is unavailable");
    const agents = (await invoke("list_managed_agents")) as MockAgent[];
    const result = (await invoke("get_channels")) as {
      channels: Array<{ id: string; name: string }>;
    };
    const channel = result.channels.find((item) => item.name === "Market Desk");
    if (!channel) throw new Error("Market Desk was not created");
    return {
      channelId: channel.id,
      rootId: (
        (await invoke("get_channel_window", {
          channelId: channel.id,
        })) as Array<{ id: string; content: string }>
      ).find((event) => event.content.includes("Analyze BTC market conditions"))
        ?.id,
      pollen: agents.find((agent) => agent.name === "Pollen")?.pubkey,
      honey: agents.find((agent) => agent.name === "Honey")?.pubkey,
    };
  });
  expect(team.rootId).toBeTruthy();
  expect(team.pollen).toBeTruthy();
  expect(team.honey).toBeTruthy();

  await page.evaluate(
    ({ channelId, pollen }) => {
      window.__BUZZ_E2E_SEED_OBSERVER_EVENTS__?.({
        agentPubkey: pollen ?? "",
        events: [
          {
            seq: Date.now(),
            timestamp: new Date().toISOString(),
            kind: "acp_read",
            agentIndex: 0,
            channelId,
            sessionId: "signaldesk-test-session",
            turnId: "signaldesk-test-turn",
            payload: {
              method: "session/update",
              params: {
                update: {
                  sessionUpdate: "agent_message_chunk",
                  content: {
                    type: "text",
                    text: "Checking current market data",
                  },
                },
              },
            },
          },
        ],
      });
    },
    { channelId: team.channelId, pollen: team.pollen },
  );
  await expect(
    workspace.getByText("Checking current market data"),
  ).toBeVisible();

  await page.evaluate(
    ({ pollen, rootId }) => {
      window.__BUZZ_E2E_EMIT_MOCK_MESSAGE__?.({
        channelName: "Market Desk",
        content:
          "Pollen market read: BTC is range bound in this mock scenario.",
        parentEventId: rootId,
        pubkey: pollen,
      });
    },
    { pollen: team.pollen, rootId: team.rootId },
  );
  await expect(workspace.getByText("Pollen responded")).toBeVisible();

  await page.evaluate(
    ({ honey, rootId }) => {
      window.__BUZZ_E2E_EMIT_MOCK_MESSAGE__?.({
        channelName: "Market Desk",
        content: "Honey record: market read noted for this request.",
        parentEventId: rootId,
        pubkey: honey,
      });
    },
    { honey: team.honey, rootId: team.rootId },
  );

  await expect(workspace.getByText("Honey responded")).toBeVisible();
  await expect(
    workspace.getByText("Pollen market read:", { exact: false }),
  ).toBeVisible();
  await expect(
    workspace.getByText("Honey record:", { exact: false }),
  ).toBeVisible();
  await expect(workspace.getByText("FINISHED")).toBeVisible();

  const taskReply = workspace.getByRole("textbox", {
    name: /REPLY IN THIS TASK/,
  });
  await taskReply.fill("@Pollen focus on volume instead");
  await workspace
    .locator("form")
    .filter({
      has: page.getByRole("textbox", { name: /REPLY IN THIS TASK/ }),
    })
    .getByRole("button", { name: "Send reply" })
    .click();
  await expect(workspace.getByText("You replied")).toBeVisible();
  const replies = await page.evaluate(
    async ({ channelId, rootId }) => {
      const invoke = (
        window as Window & {
          __TAURI_INTERNALS__?: {
            invoke: (command: string, args: unknown) => Promise<unknown>;
          };
        }
      ).__TAURI_INTERNALS__?.invoke;
      if (!invoke) throw new Error("Mock Tauri bridge is unavailable");
      return invoke("get_thread_replies", { channelId, rootEventId: rootId });
    },
    { channelId: team.channelId, rootId: team.rootId },
  );
  expect(JSON.stringify(replies)).toContain("focus on volume instead");
});

test("shows read-only bStock research in opportunities", async ({ page }) => {
  await installMockBridge(page);
  await page.goto("/");
  await expect(page.getByTestId("signaldesk-workspace")).toBeVisible();
  await page.evaluate(() => {
    const mockWindow = window as Window & {
      __SD_TEST_PRICE__?: string;
      __SD_TEST_TIME__?: number;
    };
    mockWindow.__SD_TEST_PRICE__ = "186.42";
    mockWindow.__SD_TEST_TIME__ = Date.now() - 3_000;
    const bridge = (
      window as Window & {
        __TAURI_INTERNALS__?: {
          invoke: (command: string, args?: unknown) => Promise<unknown>;
        };
      }
    ).__TAURI_INTERNALS__;
    if (!bridge) throw new Error("Mock bridge missing");
    const invoke = bridge.invoke;
    bridge.invoke = (command, args) => {
      if (command === "research_signaldesk_bstock") {
        const ticker = (args as { ticker: string }).ticker;
        return Promise.resolve({
          ticker,
          companyName: `${ticker} sample company`,
          asset: { tokenSymbol: `${ticker}B`, tokenContractAddress: "0x123" },
          price: {
            tokenPrice:
              ticker === "NVDA"
                ? (mockWindow.__SD_TEST_PRICE__ ?? "186.42")
                : "186.42",
            referencePrice: "184.00",
            tokenPriceUpdatedAt: mockWindow.__SD_TEST_TIME__,
          },
          underlyingMarket: {
            statusInfo: { openState: true, marketStatus: "OPEN" },
            marketData: { low52W: "100", high52W: "200" },
          },
        });
      }
      return invoke(command, args);
    };
  });
  await page.getByRole("button", { name: "Opportunities" }).click();
  const opportunities = page.getByTestId("signaldesk-opportunities");
  await expect(opportunities).toBeVisible();
  await expect(opportunities.getByText("$186.42").first()).toBeVisible();
  await expect(opportunities.getByText("+0.00%")).toHaveCount(0);
  await expect(
    opportunities.getByText(
      "The bStock token is 1.3% above the underlying reference.",
      {
        exact: false,
      },
    ),
  ).toBeVisible();
  await opportunities
    .getByRole("textbox", { name: "Add stock ticker" })
    .fill("AMZN");
  await opportunities.getByRole("button", { name: "Track ticker" }).click();
  await expect(opportunities.getByText("AMZN", { exact: true })).toBeVisible();
  await page.evaluate(() => {
    const mockWindow = window as Window & {
      __SD_TEST_PRICE__?: string;
      __SD_TEST_TIME__?: number;
    };
    mockWindow.__SD_TEST_PRICE__ = "205.06";
    mockWindow.__SD_TEST_TIME__ = Date.now();
  });
  await opportunities.getByRole("button", { name: "Refresh prices" }).click();
  await expect(
    opportunities.getByRole("button", { name: /NVDA/ }).getByText("+10.00%"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Market desk" }).click();
  await page.getByRole("button", { name: "Opportunities" }).click();
  await expect(
    page
      .getByTestId("signaldesk-opportunities")
      .getByRole("button", { name: /NVDA/ })
      .getByText("+10.00%"),
  ).toBeVisible();
});
