import * as React from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  Check,
  CircleHelp,
  LockKeyhole,
  Orbit,
  PlugZap,
  RotateCcw,
  Send,
  Settings2,
  Sparkles,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import { toast } from "sonner";
import {
  managedAgentsQueryKey,
  useAcpRuntimesQuery,
  useManagedAgentsQuery,
} from "@/features/agents/hooks";
import { attachManagedAgentToChannel } from "@/features/agents/channelAgents";
import { updateManagedAgent } from "@/shared/api/tauri";
import { startManagedAgent } from "@/shared/api/tauriManagedAgents";
import { useCommunities } from "@/features/communities/useCommunities";
import { initializeStarterChannels } from "@/features/onboarding/hooks";
import { useWelcomeKickoff } from "@/features/onboarding/welcomeKickoff";
import type { SettingsSection } from "@/features/settings/ui/SettingsPanels";
import { useChannelWorkingAgentPubkeys } from "@/features/agents/agentWorkingSignal";
import { SignalDeskOpportunities } from "./SignalDeskOpportunities";
import { useAgentTranscript } from "@/features/agents/ui/useObserverEvents";
import { getActivityHeadline } from "@/features/agents/ui/agentSessionTranscriptPresentation";
import {
  routeSignalDeskRequest,
  type SignalDeskAgentName,
} from "@/features/channels/lib/signalDeskRouting";
import {
  useChannelMessagesQuery,
  useChannelSubscription,
  useSendMessageMutation,
} from "@/features/messages/hooks";
import { useThreadReplies } from "@/features/messages/useThreadReplies";
import { getThreadReference } from "@/features/messages/lib/threading";
import {
  KIND_STREAM_MESSAGE,
  KIND_STREAM_MESSAGE_V2,
} from "@/shared/constants/kinds";
import type {
  Channel,
  Identity,
  ManagedAgent,
  RelayEvent,
} from "@/shared/api/types";
import "./signalDeskWorkspace.css";

const AGENTS: ReadonlyArray<{
  name: SignalDeskAgentName;
  role: string;
  symbol: string;
}> = [
  { name: "Pollen", role: "Market analyst", symbol: "✦" },
  { name: "Fizz", role: "Trading operator", symbol: "↗" },
  { name: "Honey", role: "Records keeper", symbol: "▤" },
];
const EXAMPLES = [
  "Analyze BTC market conditions",
  "Prepare a cautious ETH trade plan",
  "Find the record of my last decision",
];
const EMPTY_EVENTS: RelayEvent[] = [];

type AgentActivity = {
  name: SignalDeskAgentName;
  headline: string;
  timestamp: number;
  kind: "tool" | "thought" | "message";
  status?: string;
};

function useActivity(
  agent: ManagedAgent | undefined,
  channelId: string,
  since: number,
) {
  const transcript = useAgentTranscript(Boolean(agent), agent?.pubkey);
  return React.useMemo<AgentActivity[]>(() => {
    if (!agent || since === 0) return [];
    return transcript.flatMap((item) => {
      const timestamp = Date.parse(item.timestamp);
      if (
        item.channelId !== channelId ||
        !Number.isFinite(timestamp) ||
        timestamp < since - 2_000 ||
        (item.type !== "tool" &&
          item.type !== "thought" &&
          item.type !== "message") ||
        (item.type === "message" && item.role !== "assistant")
      )
        return [];
      const headline = getActivityHeadline(item);
      if (!headline) return [];
      return [
        {
          name: agent.name as SignalDeskAgentName,
          headline,
          timestamp,
          kind: item.type,
          status: item.type === "tool" ? item.status : undefined,
        },
      ];
    });
  }, [agent, channelId, since, transcript]);
}

function messageEvents(events: RelayEvent[]) {
  return events.filter(
    (event) =>
      event.kind === KIND_STREAM_MESSAGE ||
      event.kind === KIND_STREAM_MESSAGE_V2,
  );
}

function isKickoffMessage(event: RelayEvent) {
  return event.tags.some(
    (tag) => tag[0] === "client" && tag[1]?.startsWith("buzz-welcome-kickoff."),
  );
}

export function SignalDeskWorkspace({
  channel,
  channelsReady,
  identity,
  onSettings,
}: {
  channel: Channel | null;
  channelsReady: boolean;
  identity?: Identity;
  onSettings: (section?: SettingsSection) => void;
}) {
  const [draft, setDraft] = React.useState("");
  const [taggedPubkeys, setTaggedPubkeys] = React.useState<string[]>([]);
  const [selectedRootId, setSelectedRootId] = React.useState<string | null>(null);
  const [newTask, setNewTask] = React.useState(false);
  const [page, setPage] = React.useState<"desk" | "opportunities">("desk");
  const [setupError, setSetupError] = React.useState<string | null>(null);
  const [setupRetry, setSetupRetry] = React.useState(0);
  const queryClient = useQueryClient();
  const runtimesQuery = useAcpRuntimesQuery();
  const openClawRuntime = runtimesQuery.data?.find(
    (runtime) => runtime.id === "openclaw",
  );
  const openClawStatus = runtimesQuery.isError
    ? "STATUS UNAVAILABLE"
    : !runtimesQuery.data
      ? "CHECKING RUNTIME"
      : openClawRuntime?.availability === "available"
        ? "RUNTIME DETECTED"
        : openClawRuntime
          ? "SETUP REQUIRED"
          : "RUNTIME NOT FOUND";
  const { activeCommunity } = useCommunities();
  const setupAttempt = React.useRef<string | null>(null);
  React.useEffect(() => {
    if (channel || !channelsReady || !identity || !activeCommunity) return;
    const key = `${activeCommunity.id}:${identity.pubkey}:${setupRetry}`;
    if (setupAttempt.current === key) return;
    setupAttempt.current = key;
    void initializeStarterChannels(queryClient, {
      focus: false,
      pubkey: identity.pubkey,
      communityScope: activeCommunity.relayUrl,
    })
      .then((result) => {
        if (!result.ok) setSetupError(result.reason);
        else setSetupError(null);
      })
      .catch((error: unknown) => {
        setSetupError(
          error instanceof Error ? error.message : "The desk could not open.",
        );
      });
  }, [
    activeCommunity,
    channel,
    channelsReady,
    identity,
    queryClient,
    setupRetry,
  ]);
  const [sending, setSending] = React.useState(false);
  const agentsQuery = useManagedAgentsQuery();
  const agents = agentsQuery.data ?? [];
  const availableAgents = React.useMemo(
    () =>
      agents.filter(
        (agent) =>
          agent.relayUrl.replace(/\/$/, "") ===
          activeCommunity?.relayUrl.replace(/\/$/, ""),
      ),
    [agents, activeCommunity?.relayUrl],
  );
  const team = React.useMemo(
    () =>
      AGENTS.map(({ name }) =>
        availableAgents.find(
          (agent) => agent.name.toLowerCase() === name.toLowerCase(),
        ),
      ),
    [availableAgents],
  );
  const messagesQuery = useChannelMessagesQuery(channel);
  const channelEvents = messagesQuery.data ?? EMPTY_EVENTS;
  useChannelSubscription(channel);
  useWelcomeKickoff(channel, channelEvents);
  const sendMessage = useSendMessageMutation(channel, identity);
  const workingPubkeys = useChannelWorkingAgentPubkeys(channel?.id);
  const messages = React.useMemo(
    () => messageEvents(channelEvents),
    [channelEvents],
  );
  const taskRoots = React.useMemo(
    () => messages.filter((event) =>
      event.pubkey.toLowerCase() === identity?.pubkey.toLowerCase() &&
      getThreadReference(event.tags).parentId === null &&
      !isKickoffMessage(event),
    ),
    [identity?.pubkey, messages],
  );
  const request =
    taskRoots.find((event) => event.id === selectedRootId) ??
    taskRoots[taskRoots.length - 1];
  const threadRepliesQuery = useThreadReplies(channel, request?.id ?? null);
  const threadMessages = React.useMemo(
    () => messageEvents(threadRepliesQuery.data ?? EMPTY_EVENTS),
    [threadRepliesQuery.data],
  );
  const ownerFollowUps = React.useMemo(
    () => threadMessages.filter((event) =>
      event.pubkey.toLowerCase() === identity?.pubkey.toLowerCase(),
    ),
    [identity?.pubkey, threadMessages],
  );
  const latestOwnerMessage = ownerFollowUps[ownerFollowUps.length - 1] ?? request;
  const selectedAgents = React.useMemo(() => {
    if (!latestOwnerMessage) return [];
    const tagged = new Set(
      latestOwnerMessage.tags
        .filter((tag) => tag[0] === "p")
        .map((tag) => tag[1]?.toLowerCase()),
    );
    if (tagged.size)
      return availableAgents.filter((agent) =>
        tagged.has(agent.pubkey.toLowerCase()),
      );
    return routeSignalDeskRequest(latestOwnerMessage.content).flatMap((name) => {
      const agent = team.find(
        (candidate) => candidate?.name.toLowerCase() === name.toLowerCase(),
      );
      return agent ? [agent] : [];
    });
  }, [latestOwnerMessage, availableAgents, team]);
  const selection = selectedAgents.map((agent) => agent.name);
  const responses = React.useMemo(
    () =>
      threadMessages.filter(
        (event) =>
          request !== undefined &&
          event.created_at >= request.created_at &&
          event.id !== request.id &&
          !isKickoffMessage(event) &&
          availableAgents.some(
            (agent) =>
              agent.pubkey.toLowerCase() === event.pubkey.toLowerCase(),
          ),
      ),
    [threadMessages, request, availableAgents],
  );
  const conversation = React.useMemo(
    () => [...ownerFollowUps, ...responses].sort((a, b) =>
      a.created_at - b.created_at || a.id.localeCompare(b.id),
    ),
    [ownerFollowUps, responses],
  );
  const responded = React.useMemo(
    () => new Set(responses
      .filter((event) => event.created_at >= (latestOwnerMessage?.created_at ?? 0))
      .map((event) => event.pubkey.toLowerCase())),
    [responses, latestOwnerMessage?.created_at],
  );
  const complete =
    request !== undefined &&
    selectedAgents.length > 0 &&
    selectedAgents.every((agent) => responded.has(agent.pubkey.toLowerCase()));
  const since = latestOwnerMessage ? latestOwnerMessage.created_at * 1_000 : 0;
  const pollenActivity = useActivity(team[0], channel?.id ?? "", since);
  const fizzActivity = useActivity(team[1], channel?.id ?? "", since);
  const honeyActivity = useActivity(team[2], channel?.id ?? "", since);
  const activity = React.useMemo(
    () =>
      [...pollenActivity, ...fizzActivity, ...honeyActivity]
        .sort((a, b) => a.timestamp - b.timestamp)
        .slice(-7),
    [pollenActivity, fizzActivity, honeyActivity],
  );
  const active = selectedAgents.some((agent) =>
    workingPubkeys.includes(agent.pubkey.toLowerCase()),
  );
  const latestResponse = responses[responses.length - 1];

  const handleSend = React.useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      const content = draft.trim();
      if (!content || !channel || !identity || sending) return;
      const parentEventId = newTask ? null : request?.id ?? null;
      const explicit = availableAgents.filter((agent) =>
        content.toLowerCase().includes(`@${agent.name.toLowerCase()}`),
      );
      const names = routeSignalDeskRequest(content);
      const defaultThreadAgents = /^CONFIRM BINANCE ACTION:/i.test(content)
        ? availableAgents.filter((agent) => agent.name.toLowerCase() === "fizz")
        : selectedAgents;
      const available = taggedPubkeys.length
        ? availableAgents.filter((agent) =>
            taggedPubkeys.includes(agent.pubkey),
          )
        : explicit.length
          ? explicit
          : parentEventId && defaultThreadAgents.length
            ? defaultThreadAgents
          : names.flatMap((name) =>
              team.filter((agent): agent is ManagedAgent =>
                Boolean(
                  agent && agent.name.toLowerCase() === name.toLowerCase(),
                ),
              ),
            );
      if (!available.length) {
        toast.error("No agents are available in this workspace yet.");
        return;
      }
      setSending(true);
      try {
        const replayFloorUnix = Math.floor(Date.now() / 1_000) - 1;
        for (const agent of available) {
          let ready = agent;
          if (
            agent.respondTo === "allowlist" &&
            !agent.respondToAllowlist.some(
              (pubkey) =>
                pubkey.toLowerCase() === identity.pubkey.toLowerCase(),
            )
          ) {
            ready = (
              await updateManagedAgent({
                pubkey: agent.pubkey,
                respondToAllowlist: [
                  ...agent.respondToAllowlist,
                  identity.pubkey,
                ],
              })
            ).agent;
          }
          if (
            !channel.memberPubkeys.some(
              (pubkey) => pubkey.toLowerCase() === agent.pubkey.toLowerCase(),
            )
          ) {
            try {
              ready = (
                await attachManagedAgentToChannel(channel.id, {
                  agent: ready,
                  ensureRunning: false,
                })
              ).agent;
            } catch (error) {
              // The channel summary can lag the membership write from onboarding.
              if (
                !(error instanceof Error) ||
                !/^already a member\.?$/i.test(error.message)
              )
                throw error;
            }
          }
          if (ready.status !== "running" && ready.status !== "deployed") {
            await startManagedAgent(ready.pubkey, {
              expectedRelayUrl: activeCommunity?.relayUrl,
              expectedSignerPubkey: identity.pubkey,
              replayFloorUnix,
            });
          }
        }
        await queryClient.invalidateQueries({
          queryKey: managedAgentsQueryKey,
        });
        const sent = await sendMessage.mutateAsync({
          channelId: channel.id,
          parentEventId,
          content: `${available.map((agent) => `@${agent.name}`).join(" ")} ${content}`,
          mentionPubkeys: available.map((agent) => agent.pubkey),
        });
        if (!parentEventId) setSelectedRootId(sent.id);
        setNewTask(false);
        setDraft("");
        setTaggedPubkeys([]);
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : "Message could not be sent.",
        );
      } finally {
        setSending(false);
      }
    },
    [
      activeCommunity?.relayUrl,
      availableAgents,
      channel,
      draft,
      identity,
      newTask,
      queryClient,
      request?.id,
      sendMessage,
      sending,
      selectedAgents,
      taggedPubkeys,
      team,
    ],
  );

  return (
    <main className="sd-app" data-testid="signaldesk-workspace">
      <header className="sd-header">
        <div className="sd-brand">
          <span className="sd-brandmark">✦</span>
          <span>SignalDesk</span>
          <small>MARKET INTELLIGENCE</small>
        </div>
        <nav className="sd-main-nav" aria-label="SignalDesk pages">
          <button
            aria-current={page === "desk" ? "page" : undefined}
            onClick={() => setPage("desk")}
            type="button"
          >
            Market desk
          </button>
          <button
            aria-current={page === "opportunities" ? "page" : undefined}
            onClick={() => setPage("opportunities")}
            type="button"
          >
            Opportunities
          </button>
        </nav>
        <div className="sd-header-actions">
          <span className="sd-live">
            <span />
            {channel ? "PRIVATE DESK LIVE" : "CONNECTING TO DESK"}
          </span>
          <button
            onClick={() => onSettings()}
            type="button"
            aria-label="Open settings"
          >
            <Settings2 size={18} />
          </button>
        </div>
      </header>
      {page === "opportunities" ? (
        <div className="sd-scroll sd-opportunities-scroll">
          <SignalDeskOpportunities
            identityPubkey={identity?.pubkey}
            key={identity?.pubkey ?? "pending"}
            onOpenConnection={() => onSettings("binance-web3")}
          />
        </div>
      ) : (
        <div className="sd-scroll">
          <section className="sd-hero">
            <div className="sd-overline">
              <LockKeyhole size={13} /> YOUR PRIVATE MARKET DESK{" "}
              <span className="sd-overline-line" />
            </div>
            <h1>
              Ask the market.
              <br />
              <em>Watch the work.</em>
            </h1>
            <p>
              Bring a question or a command. SignalDesk routes it to Pollen,
              Fizz, Honey, or the whole team, then shows the work as it happens.
            </p>
          </section>
          {taskRoots.length > 0 ? (
            <section className="sd-task-switcher" aria-label="Task conversations">
              <div className="sd-task-switcher-head">
                <span>YOUR CONVERSATIONS</span>
                <button
                  className={newTask ? "is-current" : ""}
                  onClick={() => { setNewTask(true); setTaggedPubkeys([]); }}
                  type="button"
                >
                  + New task
                </button>
              </div>
              <div className="sd-task-list">
                {taskRoots.slice(-6).reverse().map((task) => (
                  <button
                    key={task.id}
                    className={!newTask && request?.id === task.id ? "is-current" : ""}
                    onClick={() => {
                      setSelectedRootId(task.id);
                      setNewTask(false);
                      setTaggedPubkeys([]);
                    }}
                    type="button"
                  >
                    {task.content.replace(/^(@\S+\s+)+/, "").slice(0, 64)}
                  </button>
                ))}
              </div>
            </section>
          ) : null}
          <form
            className="sd-composer"
            onSubmit={(event) => void handleSend(event)}
          >
            <label htmlFor="signaldesk-request">
              {newTask || !request ? "NEW TASK" : "REPLY IN THIS TASK · DIRECT, APPROVE, OR CHANGE COURSE"}
            </label>
            <div className="sd-composer-row">
              <textarea
                id="signaldesk-request"
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.shiftKey) {
                    event.preventDefault();
                    event.currentTarget.form?.requestSubmit();
                  }
                }}
                placeholder={newTask || !request
                  ? "What should the desk look into?"
                  : "Reply to the agents in this conversation…"}
                disabled={!channel || !identity || sending}
              />
              <button
                type="submit"
                disabled={!draft.trim() || !channel || !identity || sending}
              >
                <Send size={16} />
                {sending ? "Sending" : newTask || !request ? "Start task" : "Send reply"}
              </button>
            </div>
            <div className="sd-composer-foot">
              <span>Try:</span>
              {EXAMPLES.map((example) => (
                <button
                  key={example}
                  type="button"
                  onClick={() => setDraft(example)}
                >
                  {example}
                  <ArrowRight size={12} />
                </button>
              ))}
              <span className="sd-tag-label">Tag agents:</span>
              {availableAgents.map((agent) => (
                <button
                  key={agent.pubkey}
                  type="button"
                  className={`sd-agent-tag ${taggedPubkeys.includes(agent.pubkey) ? "is-selected" : ""}`}
                  aria-pressed={taggedPubkeys.includes(agent.pubkey)}
                  onClick={() =>
                    setTaggedPubkeys((current) =>
                      current.includes(agent.pubkey)
                        ? current.filter((pubkey) => pubkey !== agent.pubkey)
                        : [...current, agent.pubkey],
                    )
                  }
                >
                  @{agent.name}
                </button>
              ))}
            </div>
            {setupError && !channel ? (
              <div className="sd-setup-error" role="alert">
                <span>{setupError}</span>
                <button
                  type="button"
                  onClick={() => {
                    setSetupError(null);
                    setSetupRetry((value) => value + 1);
                  }}
                >
                  <RotateCcw size={13} /> Retry desk setup
                </button>
              </div>
            ) : null}
          </form>
          <section className="sd-workflow" aria-label="Live request workflow">
            <div className="sd-section-head">
              <span>01 / THE ROUTE</span>
              <strong>
                {request
                  ? complete
                    ? "COMPLETE"
                    : active
                      ? "AGENTS WORKING"
                      : "REQUEST RECEIVED"
                  : "READY FOR YOUR REQUEST"}
              </strong>
            </div>
            <div
              className={`sd-route-stage ${request ? "has-request" : ""} ${complete ? "is-complete" : ""}`}
            >
              <div className="sd-request-card">
                <span>YOUR MESSAGE</span>
                <p>
                  {latestOwnerMessage?.content.replace(
                    /^(@(?:Pollen|Fizz|Honey)\s+)+/i,
                    "",
                  ) ?? "Your request enters here."}
                </p>
                <small>
                  {latestOwnerMessage
                    ? new Date(latestOwnerMessage.created_at * 1_000).toLocaleTimeString(
                        [],
                        { hour: "2-digit", minute: "2-digit" },
                      )
                    : "Awaiting input"}
                </small>
              </div>
              <div className="sd-route-line sd-line-first">
                <i />
              </div>
              <div className="sd-core">
                <span>✦</span>
                <small>ROUTING</small>
              </div>
              <div className="sd-route-line sd-line-second">
                <i />
              </div>
              <div className="sd-finish">
                <Check size={23} />
                <small>{complete ? "DONE" : "OUTPUT"}</small>
              </div>
            </div>
            <section className="sd-agent-row" aria-label="Agent flow map">
              <svg
                className="sd-agent-map-lines"
                viewBox="0 0 1000 660"
                preserveAspectRatio="none"
                aria-hidden="true"
              >
                <path d="M286 110 C420 110 400 315 500 315 S590 315 700 330" />
                <path d="M700 330 C570 330 610 550 290 550" />
                <circle cx="500" cy="315" r="7" />
              </svg>
              <div className="sd-map-hub" aria-hidden="true">
                <span>◇</span>
                <small>DESK ROUTER</small>
              </div>
              {AGENTS.map(({ name, role, symbol }, index) => {
                const agent = team[index];
                const selected = selection.includes(name);
                const working = Boolean(
                  agent && workingPubkeys.includes(agent.pubkey.toLowerCase()),
                );
                const done = Boolean(
                  agent && responded.has(agent.pubkey.toLowerCase()),
                );
                return (
                  <article
                    className={`sd-agent-card ${selected ? "is-selected" : ""} ${working ? "is-working" : ""} ${done ? "is-done" : ""}`}
                    key={name}
                  >
                    <div className="sd-agent-top">
                      <span className="sd-agent-symbol">{symbol}</span>
                      <span className="sd-agent-index">0{index + 1}</span>
                    </div>
                    <h3>{name}</h3>
                    <p>{role}</p>
                    <div className="sd-agent-chart" aria-hidden>
                      <svg viewBox="0 0 160 36" preserveAspectRatio="none">
                        <title>Market activity motif</title>
                        <path d="M0 29L17 22L29 25L43 14L56 18L69 8L81 15L94 9L108 17L122 5L138 11L160 3" />
                      </svg>
                    </div>
                    <div className="sd-agent-state">
                      <span className="sd-agent-dot" />
                      {!agent
                        ? "Starting"
                        : done
                          ? "Response ready"
                          : working
                            ? "Working now"
                            : agent.status !== "running" &&
                                agent.status !== "deployed"
                              ? "Starting"
                              : selected
                                ? "Routed"
                                : "On standby"}
                    </div>
                  </article>
                );
              })}
            </section>
          </section>
          <section className="sd-lower">
            <div className="sd-activity">
              <div className="sd-section-head">
                <span>02 / CODEX + ACP ACTIVITY</span>
                <strong>
                  {active
                    ? "STREAMING"
                    : activity.length
                      ? "LATEST RUN"
                      : "STANDBY"}
                </strong>
              </div>
              <div className="sd-activity-body">
                {activity.length ? (
                  activity.map((item) => (
                    <div
                      className="sd-activity-item"
                      key={`${item.name}-${item.timestamp}-${item.kind}-${item.headline}`}
                    >
                      <span className="sd-activity-bullet" />
                      <div>
                        <strong>
                          {item.name}{" "}
                          <small>
                            {item.kind === "tool"
                              ? (item.status ?? "tool")
                              : item.kind}
                          </small>
                        </strong>
                        <p>{item.headline}</p>
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="sd-empty-activity">
                    <Sparkles size={21} />
                    <p>
                      Agent steps and tool progress will appear here after you
                      send a request.
                    </p>
                  </div>
                )}
              </div>
            </div>
            <div className="sd-result">
              <div className="sd-section-head">
                <span>03 / RETURNED TO YOUR DESK</span>
                <strong>
                  {complete
                    ? "FINISHED"
                    : latestResponse
                      ? "RESPONSE ARRIVING"
                      : "AWAITING RESPONSE"}
                </strong>
              </div>
              <div className="sd-result-body">
                {conversation.length ? (
                  conversation.map((response) => {
                    const isOwner = response.pubkey.toLowerCase() === identity?.pubkey.toLowerCase();
                    const responseAgent = availableAgents.find(
                      (agent) =>
                        agent.pubkey.toLowerCase() ===
                        response.pubkey.toLowerCase(),
                    );
                    return (
                      <article className={`sd-result-reply ${isOwner ? "is-owner" : ""}`} key={response.id}>
                        <div className="sd-result-author">
                          <span className="sd-result-check">
                            {isOwner ? <Send size={13} /> : <Check size={15} />}
                          </span>
                          {isOwner ? "You replied" : `${responseAgent?.name ?? "The team"} responded`}
                        </div>
                        <div className="sd-result-copy">
                          <ReactMarkdown>{response.content}</ReactMarkdown>
                        </div>
                      </article>
                    );
                  })
                ) : (
                  <div className="sd-result-empty">
                    <CircleHelp size={23} />
                    <h3>
                      {request
                        ? "The team is on it."
                        : "A finished answer lands here."}
                    </h3>
                    <p>
                      {request
                        ? "The conversation and final output will appear as agents respond."
                        : "Your request, agent activity, and answer stay together in your private desk."}
                    </p>
                  </div>
                )}
              </div>
              {request && !newTask ? (
                <form
                  className="sd-composer sd-thread-composer"
                  onSubmit={(event) => void handleSend(event)}
                >
                  <label htmlFor="signaldesk-thread-reply">
                    REPLY IN THIS CONVERSATION · DIRECT, APPROVE, OR CHANGE COURSE
                  </label>
                  <div className="sd-composer-row">
                    <textarea
                      id="signaldesk-thread-reply"
                      value={draft}
                      onChange={(event) => setDraft(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" && !event.shiftKey) {
                          event.preventDefault();
                          event.currentTarget.form?.requestSubmit();
                        }
                      }}
                      placeholder="Reply to the agents in this task…"
                      disabled={!channel || !identity || sending}
                    />
                    <button
                      type="submit"
                      disabled={!draft.trim() || !channel || !identity || sending}
                    >
                      <Send size={16} /> {sending ? "Sending" : "Send reply"}
                    </button>
                  </div>
                </form>
              ) : null}
            </div>
          </section>
          <section className="sd-openclaw-slot">
            <span className="sd-openclaw-icon">
              <PlugZap size={19} />
            </span>
            <div>
              <small>EXPANSION PORT / OPENCLAW</small>
              <strong>Connect OpenClaw through the agent runtime.</strong>
              <p>
                {openClawRuntime?.availability === "available"
                  ? "The OpenClaw command is available on this device. Configure its Gateway and agent routing in settings before using it here."
                  : "Set up the OpenClaw runtime and Gateway in Agents settings. Connection and agent permissions can be reviewed there."}
              </p>
            </div>
            <div className="sd-openclaw-actions">
              <span className="sd-openclaw-status">
                <Orbit size={14} /> {openClawStatus}
              </span>
              <button onClick={() => onSettings("agents")} type="button">
                Open agent settings <ArrowRight size={13} />
              </button>
            </div>
          </section>
          <footer className="sd-footer">
            <span>
              <LockKeyhole size={12} /> Private workspace
            </span>
            <span>
              Account actions require a separate connection and your
              confirmation.
            </span>
            {messagesQuery.isError ? (
              <button
                onClick={() => void messagesQuery.refetch()}
                type="button"
              >
                <RotateCcw size={13} /> Retry conversation
              </button>
            ) : null}
          </footer>
        </div>
      )}
    </main>
  );
}
