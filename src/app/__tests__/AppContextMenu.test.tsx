import "@/lib/i18n";

import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import i18n from "@/lib/i18n";
import { useNavigationStore } from "@/stores/navigationStore";

import { AppContextMenu } from "../AppContextMenu";

// jsdom no implementa `execCommand`/`navigator.clipboard` — se reemplazan
// por dobles de prueba, igual que se mockearía cualquier otra API del
// navegador ausente en jsdom (ver AvatarEditor.tsx's uso real de
// `navigator.clipboard.writeText`, sin test propio que lo ejercite).
beforeEach(() => {
  document.execCommand = vi.fn().mockReturnValue(true);
  Object.defineProperty(navigator, "clipboard", {
    value: { readText: vi.fn().mockResolvedValue("") },
    configurable: true,
  });
  useNavigationStore.setState({ isSettingsOpen: false });
});

describe("AppContextMenu", () => {
  it("renders nothing until the user right-clicks", () => {
    render(<AppContextMenu />);
    expect(screen.queryByText(i18n.t("contextMenu.settings"))).not.toBeInTheDocument();
  });

  it("adds Inspeccionar to the same menu on Shift+right-click, without dropping the rest", () => {
    render(<AppContextMenu />);

    fireEvent.contextMenu(document.body, { shiftKey: true });

    expect(screen.getByText(i18n.t("contextMenu.settings"))).toBeInTheDocument();
    expect(screen.getByText(i18n.t("contextMenu.inspect"))).toBeInTheDocument();
  });

  it("does not offer Inspeccionar on a plain right-click", async () => {
    const user = userEvent.setup();
    render(<AppContextMenu />);

    await user.pointer({ target: document.body, keys: "[MouseRight]" });

    expect(screen.queryByText(i18n.t("contextMenu.inspect"))).not.toBeInTheDocument();
  });

  it("shows the default app actions on a non-editable right-click", async () => {
    const user = userEvent.setup();
    render(<AppContextMenu />);

    await user.pointer({ target: document.body, keys: "[MouseRight]" });

    expect(screen.getByText(i18n.t("contextMenu.settings"))).toBeInTheDocument();
    expect(screen.queryByText(i18n.t("contextMenu.cut"))).not.toBeInTheDocument();
    expect(screen.queryByText(i18n.t("contextMenu.paste"))).not.toBeInTheDocument();
  });

  it("shows edit actions (cut/copy disabled) on an empty input", async () => {
    const user = userEvent.setup();
    render(
      <>
        <input aria-label="demo" />
        <AppContextMenu />
      </>,
    );

    await user.pointer({ target: screen.getByLabelText("demo"), keys: "[MouseRight]" });

    expect(screen.getByText(i18n.t("contextMenu.cut"))).toBeInTheDocument();
    expect(screen.getByText(i18n.t("contextMenu.copy"))).toBeInTheDocument();
    expect(screen.getByText(i18n.t("contextMenu.paste"))).toBeInTheDocument();
    expect(screen.getByText(i18n.t("contextMenu.selectAll"))).toBeInTheDocument();
    expect(screen.getByText(i18n.t("contextMenu.cut")).closest("button")).toBeDisabled();
    expect(screen.getByText(i18n.t("contextMenu.copy")).closest("button")).toBeDisabled();
  });

  it("enables cut/copy when the input has a text selection", () => {
    render(
      <>
        <input aria-label="demo" defaultValue="hola mundo" />
        <AppContextMenu />
      </>,
    );

    // `fireEvent` en vez de `userEvent.pointer`: un click real simulado
    // incluye su propio `mousedown`, que colapsaría la selección al mover
    // el caret a la posición del click (mismo comportamiento que un
    // navegador real) — acá se prueba solo la lógica del menú a partir de
    // una selección ya existente, no la interacción completa del mouse.
    const input = screen.getByLabelText("demo") as HTMLInputElement;
    input.setSelectionRange(0, 4);
    input.focus();
    fireEvent.contextMenu(input, { clientX: 10, clientY: 10 });

    expect(screen.getByText(i18n.t("contextMenu.copy")).closest("button")).not.toBeDisabled();
  });

  it("clicking Seleccionar todo runs the matching execCommand", async () => {
    const user = userEvent.setup();
    render(
      <>
        <input aria-label="demo" />
        <AppContextMenu />
      </>,
    );

    await user.pointer({ target: screen.getByLabelText("demo"), keys: "[MouseRight]" });
    await user.click(screen.getByText(i18n.t("contextMenu.selectAll")));

    expect(document.execCommand).toHaveBeenCalledWith("selectAll");
    expect(screen.queryByText(i18n.t("contextMenu.selectAll"))).not.toBeInTheDocument();
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<AppContextMenu />);

    await user.pointer({ target: document.body, keys: "[MouseRight]" });
    expect(screen.getByText(i18n.t("contextMenu.settings"))).toBeInTheDocument();

    await user.keyboard("{Escape}");

    expect(screen.queryByText(i18n.t("contextMenu.settings"))).not.toBeInTheDocument();
  });

  it("offers Copiar mensaje on a chat bubble and copies its raw content", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: { readText: vi.fn().mockResolvedValue(""), writeText },
      configurable: true,
    });
    render(
      <>
        <div data-copy-text="hola **mundo**">
          <p>hola mundo</p>
        </div>
        <AppContextMenu />
      </>,
    );

    await user.pointer({ target: screen.getByText("hola mundo"), keys: "[MouseRight]" });
    expect(screen.getByText(i18n.t("contextMenu.copyMessage"))).toBeInTheDocument();

    await user.click(screen.getByText(i18n.t("contextMenu.copyMessage")));

    expect(writeText).toHaveBeenCalledWith("hola **mundo**");
  });

  it("does not offer Copiar mensaje outside a chat bubble", async () => {
    const user = userEvent.setup();
    render(<AppContextMenu />);

    await user.pointer({ target: document.body, keys: "[MouseRight]" });

    expect(screen.queryByText(i18n.t("contextMenu.copyMessage"))).not.toBeInTheDocument();
  });

  it("stays clickable even when a modal Dialog sets body pointer-events to none", async () => {
    const user = userEvent.setup();
    render(<AppContextMenu />);

    await user.pointer({ target: document.body, keys: "[MouseRight]" });

    // Reproduce exactamente lo que hace `@radix-ui/react-dismissable-layer`
    // mientras un `Dialog` modal (Configuración, Command Palette...) está
    // abierto — sin `pointer-events-auto` en el propio menú, este click no
    // llegaba a registrarse (bug reportado: "el menú aparece pero no deja
    // clickear las opciones").
    document.body.style.pointerEvents = "none";
    try {
      await user.click(screen.getByText(i18n.t("contextMenu.settings")));
    } finally {
      document.body.style.pointerEvents = "";
    }

    expect(useNavigationStore.getState().isSettingsOpen).toBe(true);
  });

  it("clicking Configuración opens the settings dialog", async () => {
    const user = userEvent.setup();
    render(<AppContextMenu />);

    await user.pointer({ target: document.body, keys: "[MouseRight]" });
    await user.click(screen.getByText(i18n.t("contextMenu.settings")));

    expect(useNavigationStore.getState().isSettingsOpen).toBe(true);
  });
});
