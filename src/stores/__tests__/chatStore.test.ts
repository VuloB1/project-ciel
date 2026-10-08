import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc/chat", () => ({
  listConversations: vi.fn(),
  createConversation: vi.fn(),
  getMessages: vi.fn(),
  renameConversation: vi.fn(),
  archiveConversation: vi.fn(),
  deleteConversation: vi.fn(),
  cancelGeneration: vi.fn(),
  sendMessage: vi.fn(),
}));

import * as chatIpc from "@/lib/ipc/chat";
import { useChatStore } from "../chatStore";

const CONVERSATION_ID = "conv-1";
const REQUEST_ID = "req-1";
const MESSAGE_ID = "msg-1";

describe("useChatStore.handleAppEvent", () => {
  beforeEach(() => {
    useChatStore.setState({
      conversationsByProfile: {},
      activeConversationId: null,
      messagesByConversation: {},
      generatingConversations: {},
      requestIndex: {},
      isLoadingConversations: false,
    });
  });

  it("started creates a streaming placeholder and marks the conversation as generating", () => {
    useChatStore.getState().handleAppEvent({
      type: "ai_response_started",
      requestId: REQUEST_ID,
      profileId: "p1",
      conversationId: CONVERSATION_ID,
      messageId: MESSAGE_ID,
    });

    const state = useChatStore.getState();
    expect(state.generatingConversations[CONVERSATION_ID]).toBe(true);
    expect(state.requestIndex[REQUEST_ID]).toEqual({ conversationId: CONVERSATION_ID, messageId: MESSAGE_ID });

    const messages = state.messagesByConversation[CONVERSATION_ID];
    expect(messages).toHaveLength(1);
    expect(messages?.[0]).toMatchObject({ id: MESSAGE_ID, role: "assistant", content: "", status: "streaming" });
  });

  it("tokens accumulate onto the placeholder message in order", () => {
    const store = useChatStore.getState();
    store.handleAppEvent({
      type: "ai_response_started",
      requestId: REQUEST_ID,
      profileId: "p1",
      conversationId: CONVERSATION_ID,
      messageId: MESSAGE_ID,
    });
    store.handleAppEvent({ type: "ai_response_token", requestId: REQUEST_ID, token: "Hola" });
    store.handleAppEvent({ type: "ai_response_token", requestId: REQUEST_ID, token: " mundo" });

    const messages = useChatStore.getState().messagesByConversation[CONVERSATION_ID];
    expect(messages?.[0]?.content).toBe("Hola mundo");
  });

  it("finished marks the message complete and clears generation tracking", () => {
    const store = useChatStore.getState();
    store.handleAppEvent({
      type: "ai_response_started",
      requestId: REQUEST_ID,
      profileId: "p1",
      conversationId: CONVERSATION_ID,
      messageId: MESSAGE_ID,
    });
    store.handleAppEvent({ type: "ai_response_token", requestId: REQUEST_ID, token: "Hola" });
    store.handleAppEvent({ type: "ai_response_finished", requestId: REQUEST_ID });

    const state = useChatStore.getState();
    expect(state.generatingConversations[CONVERSATION_ID]).toBeUndefined();
    expect(state.requestIndex[REQUEST_ID]).toBeUndefined();
    expect(state.messagesByConversation[CONVERSATION_ID]?.[0]).toMatchObject({ content: "Hola", status: "complete" });
  });

  it("error replaces the content with the error message and marks status error", () => {
    const store = useChatStore.getState();
    store.handleAppEvent({
      type: "ai_response_started",
      requestId: REQUEST_ID,
      profileId: "p1",
      conversationId: CONVERSATION_ID,
      messageId: MESSAGE_ID,
    });
    store.handleAppEvent({ type: "ai_response_error", requestId: REQUEST_ID, message: "Connection refused" });

    const message = useChatStore.getState().messagesByConversation[CONVERSATION_ID]?.[0];
    expect(message).toMatchObject({ status: "error", content: "Connection refused" });
  });

  it("cancelled marks the message cancelled without touching its content", () => {
    const store = useChatStore.getState();
    store.handleAppEvent({
      type: "ai_response_started",
      requestId: REQUEST_ID,
      profileId: "p1",
      conversationId: CONVERSATION_ID,
      messageId: MESSAGE_ID,
    });
    store.handleAppEvent({ type: "ai_response_token", requestId: REQUEST_ID, token: "Parcial" });
    store.handleAppEvent({ type: "ai_response_cancelled", requestId: REQUEST_ID });

    const message = useChatStore.getState().messagesByConversation[CONVERSATION_ID]?.[0];
    expect(message).toMatchObject({ status: "cancelled", content: "Parcial" });
  });

  it("a token for an unknown request id is ignored rather than throwing", () => {
    expect(() =>
      useChatStore.getState().handleAppEvent({ type: "ai_response_token", requestId: "unknown", token: "x" }),
    ).not.toThrow();
    expect(useChatStore.getState().messagesByConversation).toEqual({});
  });
});

describe("useChatStore.send", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useChatStore.setState({
      conversationsByProfile: {},
      activeConversationId: null,
      messagesByConversation: {},
      generatingConversations: {},
      requestIndex: {},
      isLoadingConversations: false,
    });
  });

  it("forwards the image to the IPC layer and stores the returned attachment on the message", async () => {
    vi.mocked(chatIpc.sendMessage).mockResolvedValue({
      id: MESSAGE_ID,
      conversationId: CONVERSATION_ID,
      role: "user",
      content: "¿Qué ves?",
      status: "complete",
      providerId: null,
      model: null,
      inputTokens: null,
      outputTokens: null,
      createdAt: "2026-01-01T00:00:00Z",
      attachmentId: "att-1",
      attachmentMimeType: "image/png",
    });

    const image = { base64: "AAAA", mimeType: "image/png", extension: "png" };
    await useChatStore.getState().send("profile-1", CONVERSATION_ID, "¿Qué ves?", image);

    expect(chatIpc.sendMessage).toHaveBeenCalledWith("profile-1", CONVERSATION_ID, "¿Qué ves?", image);
    const stored = useChatStore.getState().messagesByConversation[CONVERSATION_ID]?.[0];
    expect(stored).toMatchObject({ attachmentId: "att-1", attachmentMimeType: "image/png" });
    expect(useChatStore.getState().generatingConversations[CONVERSATION_ID]).toBe(true);
  });
});
