import "@/lib/i18n";

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DEFAULT_IDENTITY, DEFAULT_PERSONALITY, DEFAULT_VOICE, type Profile } from "@/types/profile";
import type { Message } from "@/types/chat";

import { MessageBubble } from "../MessageBubble";

function makeMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: "m1",
    conversationId: "c1",
    role: "assistant",
    content: "",
    status: "complete",
    providerId: null,
    model: null,
    inputTokens: null,
    outputTokens: null,
    createdAt: "2026-01-01T00:00:00Z",
    attachmentId: null,
    attachmentMimeType: null,
    ...overrides,
  };
}

function makeProfile(overrides: Partial<Profile> = {}): Profile {
  return {
    id: "p1",
    name: "Ciel",
    nickname: null,
    avatarPath: null,
    description: null,
    ageFictional: null,
    pronouns: null,
    occupation: null,
    species: null,
    accentColor: null,
    theme: null,
    isArchived: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    identity: DEFAULT_IDENTITY,
    personality: DEFAULT_PERSONALITY,
    voice: DEFAULT_VOICE,
    avatarIdlePath: null,
    avatarThinkingPath: null,
    avatarTalkingPath: null,
    ...overrides,
  };
}

describe("MessageBubble", () => {
  it("renders **bold** text as a real <strong>, not literal asterisks", () => {
    render(<MessageBubble message={makeMessage({ content: "esto es **importante**" })} profile={makeProfile()} />);

    const strong = screen.getByText("importante");
    expect(strong.tagName).toBe("STRONG");
    expect(screen.queryByText(/\*\*/)).not.toBeInTheDocument();
  });

  it("renders a Markdown link as a real, clickable anchor that opens in a new tab", () => {
    render(<MessageBubble message={makeMessage({ content: "mirá [VGen](https://vgen.co/ryonikku)" })} profile={makeProfile()} />);

    const link = screen.getByRole("link", { name: "VGen" });
    expect(link).toHaveAttribute("href", "https://vgen.co/ryonikku");
    expect(link).toHaveAttribute("target", "_blank");
    expect(screen.queryByText(/vgen\.co/)).not.toBeInTheDocument();
  });

  it("renders a list with real list items, not literal dashes", () => {
    render(<MessageBubble message={makeMessage({ content: "- primero\n- segundo" })} profile={makeProfile()} />);

    expect(screen.getByText("primero").closest("li")).not.toBeNull();
    expect(screen.getByText("segundo").closest("li")).not.toBeNull();
  });

  it("shows the character's avatar next to their own message but not next to the user's", () => {
    const { rerender } = render(
      <MessageBubble message={makeMessage({ role: "assistant", content: "hola" })} profile={makeProfile({ name: "Ciel" })} />,
    );
    // No avatar file set — ProfileAvatarBadge falls back to the name's initial.
    expect(screen.getByText("C")).toBeInTheDocument();

    rerender(<MessageBubble message={makeMessage({ role: "user", content: "hola" })} profile={makeProfile({ name: "Ciel" })} />);
    expect(screen.queryByText("C")).not.toBeInTheDocument();
  });
});
