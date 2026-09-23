import { fireEvent, render, screen } from "@testing-library/svelte";
import { get } from "svelte/store";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  updateConfig: vi.fn<(patch: Record<string, unknown>) => Promise<void>>(
    async () => undefined,
  ),
  getConfig: vi.fn<() => Promise<{ game_path: string; modpack_url: string }>>(
    async () => ({ game_path: "", modpack_url: "" }),
  ),
  setGamePath: vi.fn<(path: string) => Promise<void>>(async () => undefined),
  setModpackUrl: vi.fn<(url: string) => Promise<void>>(async () => undefined),
  fetchModpackJson: vi.fn<(url: string) => Promise<{ version: string }>>(
    async () => ({ version: "1.0.0" }),
  ),
  detectGamePath: vi.fn<() => Promise<string | null>>(
    async (): Promise<string | null> => null,
  ),
  logout: vi.fn<() => Promise<void>>(async () => undefined),
  validateAndSaveModioApiKey: vi.fn<(value: string) => Promise<boolean>>(
    async () => true,
  ),
  validateAndSaveModioToken: vi.fn<(value: string) => Promise<boolean>>(
    async () => true,
  ),
  validateAndSaveNexusApiKey: vi.fn<(value: string) => Promise<boolean>>(
    async () => true,
  ),
  openDialog: vi.fn<(options?: unknown) => Promise<string | null>>(
    async (): Promise<string | null> => null,
  ),
  openUrl: vi.fn<(url: string) => Promise<void>>(async () => undefined),
  toastSuccess: vi.fn<(message: string) => void>(),
  tokenSet: vi.fn<(value: boolean) => void>(),
}));

vi.mock("../../../src/lib/api/commands", () => ({
  getConfig: mocks.getConfig,
  setGamePath: mocks.setGamePath,
  setModpackUrl: mocks.setModpackUrl,
  fetchModpackJson: mocks.fetchModpackJson,
  detectGamePath: mocks.detectGamePath,
  logout: mocks.logout,
  updateConfig: mocks.updateConfig,
}));
vi.mock("../../../src/lib/api/apiKeyValidation", () => ({
  validateAndSaveModioApiKey: mocks.validateAndSaveModioApiKey,
  validateAndSaveModioToken: mocks.validateAndSaveModioToken,
  validateAndSaveNexusApiKey: mocks.validateAndSaveNexusApiKey,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.openDialog }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: mocks.openUrl }));
vi.mock("../../../src/lib/stores/toast", () => ({
  toastStore: {
    success: mocks.toastSuccess,
    error: vi.fn<(message: string) => void>(),
  },
}));
vi.mock("../../../src/lib/stores/token", () => ({
  tokenStore: { set: mocks.tokenSet },
}));

import SetupWizard from "../../../src/lib/components/SetupWizard.svelte";
import { callTourAction } from "../../../src/lib/tour/registry";
import {
  setSetupWizardPage,
  setupWizardPage,
} from "../../../src/lib/stores/setupWizard";

beforeEach(() => {
  vi.clearAllMocks();
  setSetupWizardPage(1);
});

describe("SetupWizard", () => {
  it("renders the setup page the tour's card asked for", () => {
    setSetupWizardPage(2);
    render(SetupWizard);

    expect(screen.getByLabelText("mod.io API Access")).not.toBeNull();
    expect(
      screen.getByLabelText("mod.io Personal Access Token"),
    ).not.toBeNull();
    // Nothing dialog-shaped: the card brings the title, progress and Back/Next.
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByText("Next")).toBeNull();
    expect(screen.queryByText("Back")).toBeNull();
  });

  it("shows the game folder fields on the first page", () => {
    render(SetupWizard);

    expect(screen.getByLabelText("Game path")).not.toBeNull();
    expect(screen.getByText("Auto Detect")).not.toBeNull();
    expect(screen.getByText("Browse...")).not.toBeNull();
  });

  it("saves the game path from the card's Next", async () => {
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Game path"), {
      target: { value: "/games/Ready Or Not" },
    });

    await expect(callTourAction("setup:save-game-path")).resolves.toBe(true);

    expect(mocks.setGamePath).toHaveBeenCalledWith("/games/Ready Or Not");
    expect(get(setupWizardPage)).toBe(2);
  });

  it("skips the game path when the box is left empty", async () => {
    render(SetupWizard);

    await expect(callTourAction("setup:save-game-path")).resolves.toBe(true);

    expect(mocks.setGamePath).not.toHaveBeenCalled();
    expect(get(setupWizardPage)).toBe(2);
  });

  it("keeps the card where it is when the keys are refused", async () => {
    mocks.validateAndSaveModioApiKey.mockResolvedValueOnce(false);
    setSetupWizardPage(2);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("mod.io API Access"), {
      target: { value: "key" },
    });
    await fireEvent.input(
      screen.getByLabelText("mod.io Personal Access Token"),
      { target: { value: "token" } },
    );

    await expect(callTourAction("setup:save-modio")).resolves.toBe(false);

    expect(screen.getByText(/API Access key is invalid/)).not.toBeNull();
    expect(get(setupWizardPage)).toBe(2);
  });

  it("moves on once mod.io accepts both values", async () => {
    setSetupWizardPage(2);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("mod.io API Access"), {
      target: { value: "key" },
    });
    await fireEvent.input(
      screen.getByLabelText("mod.io Personal Access Token"),
      { target: { value: "token" } },
    );

    await expect(callTourAction("setup:save-modio")).resolves.toBe(true);

    expect(get(setupWizardPage)).toBe(3);
  });

  it("skips Nexus when the box is left empty", async () => {
    setSetupWizardPage(3);
    render(SetupWizard);

    await expect(callTourAction("setup:save-nexus")).resolves.toBe(true);

    expect(mocks.validateAndSaveNexusApiKey).not.toHaveBeenCalled();
    expect(get(setupWizardPage)).toBe(4);
  });

  it("saves the modpack URL and finishes setup", async () => {
    setSetupWizardPage(4);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Modpack URL"), {
      target: { value: "https://example.com/modpack.json" },
    });

    await expect(callTourAction("setup:finish")).resolves.toBe(true);

    expect(mocks.fetchModpackJson).toHaveBeenCalledWith(
      "https://example.com/modpack.json",
    );
    expect(mocks.setModpackUrl).toHaveBeenCalledWith(
      "https://example.com/modpack.json",
    );
    expect(mocks.updateConfig).toHaveBeenCalledWith({
      setup_wizard_complete: true,
    });
  });

  it("reports a modpack URL it could not fetch", async () => {
    mocks.fetchModpackJson.mockRejectedValueOnce(new Error("404"));
    setSetupWizardPage(4);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Modpack URL"), {
      target: { value: "https://example.com/nope.json" },
    });

    await expect(callTourAction("setup:finish")).resolves.toBe(false);

    expect(screen.getByText(/Could not fetch modpack/)).not.toBeNull();
    expect(get(setupWizardPage)).toBe(4);
  });

  it("settles setup when the user picks Set up later", async () => {
    setSetupWizardPage(2);
    render(SetupWizard);

    await callTourAction("setup:defer");

    expect(mocks.updateConfig).toHaveBeenCalledWith({
      setup_wizard_complete: true,
    });
    expect(mocks.toastSuccess).toHaveBeenCalledTimes(1);
  });

  it("loads the game path the app already has", async () => {
    mocks.getConfig.mockResolvedValueOnce({
      game_path: "/games/Ready Or Not",
      modpack_url: "https://example.com/modpack.json",
    });
    render(SetupWizard);

    const field = screen.getByLabelText("Game path") as HTMLInputElement;
    await vi.waitFor(() => {
      expect(field.value).toBe("/games/Ready Or Not");
    });
  });

  it("auto-detects the game path on mount", async () => {
    mocks.detectGamePath.mockResolvedValueOnce("/auto/game");
    render(SetupWizard);

    const field = screen.getByLabelText("Game path") as HTMLInputElement;
    await vi.waitFor(() => {
      expect(field.value).toBe("/auto/game");
    });
  });

  it("tolerates a failed config read and failed detection on mount", async () => {
    mocks.getConfig.mockRejectedValueOnce(new Error("config down"));
    mocks.detectGamePath.mockRejectedValueOnce(new Error("detect down"));
    render(SetupWizard);

    await vi.waitFor(() => {
      expect(mocks.detectGamePath).toHaveBeenCalled();
    });
    expect((screen.getByLabelText("Game path") as HTMLInputElement).value).toBe(
      "",
    );
  });

  it("fills the path from the Auto Detect button", async () => {
    render(SetupWizard);
    await vi.waitFor(() => {
      expect(mocks.detectGamePath).toHaveBeenCalled();
    });
    mocks.detectGamePath.mockResolvedValueOnce("/detected/game");

    await fireEvent.click(screen.getByText("Auto Detect"));

    const field = screen.getByLabelText("Game path") as HTMLInputElement;
    await vi.waitFor(() => {
      expect(field.value).toBe("/detected/game");
    });
  });

  it("reports when auto-detect finds nothing", async () => {
    render(SetupWizard);
    await vi.waitFor(() => {
      expect(mocks.detectGamePath).toHaveBeenCalled();
    });

    await fireEvent.click(screen.getByText("Auto Detect"));

    expect(
      await screen.findByText(/Could not auto-detect game path/),
    ).not.toBeNull();
  });

  it("reports when auto-detect throws", async () => {
    render(SetupWizard);
    await vi.waitFor(() => {
      expect(mocks.detectGamePath).toHaveBeenCalled();
    });
    mocks.detectGamePath.mockRejectedValueOnce(new Error("no steam"));

    await fireEvent.click(screen.getByText("Auto Detect"));

    expect(await screen.findByText(/Auto-detect failed/)).not.toBeNull();
  });

  it("fills the path from the browse dialog", async () => {
    mocks.openDialog.mockResolvedValueOnce("/picked/game");
    render(SetupWizard);

    await fireEvent.click(screen.getByText("Browse..."));

    const field = screen.getByLabelText("Game path") as HTMLInputElement;
    await vi.waitFor(() => {
      expect(field.value).toBe("/picked/game");
    });
    expect(mocks.openDialog).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true, defaultPath: undefined }),
    );
  });

  it("passes the current path to the browse dialog", async () => {
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Game path"), {
      target: { value: "/games/Ready Or Not" },
    });
    mocks.openDialog.mockResolvedValueOnce(null);

    await fireEvent.click(screen.getByText("Browse..."));

    expect(mocks.openDialog).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: "/games/Ready Or Not" }),
    );
    expect((screen.getByLabelText("Game path") as HTMLInputElement).value).toBe(
      "/games/Ready Or Not",
    );
  });

  it("reports when browsing for a path fails", async () => {
    mocks.openDialog.mockRejectedValueOnce(new Error("dialog broke"));
    render(SetupWizard);

    await fireEvent.click(screen.getByText("Browse..."));

    expect(await screen.findByText(/Browse failed/)).not.toBeNull();
  });

  it("reports when saving the game path fails", async () => {
    mocks.setGamePath.mockRejectedValueOnce(new Error("denied"));
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Game path"), {
      target: { value: "/games/Ready Or Not" },
    });

    await expect(callTourAction("setup:save-game-path")).resolves.toBe(false);

    expect(await screen.findByText(/denied/)).not.toBeNull();
    expect(get(setupWizardPage)).toBe(1);
  });

  it("requires the mod.io API key", async () => {
    setSetupWizardPage(2);
    render(SetupWizard);

    await expect(callTourAction("setup:save-modio")).resolves.toBe(false);

    expect(
      await screen.findByText(/Please enter your mod.io API Access key/),
    ).not.toBeNull();
  });

  it("requires the mod.io token", async () => {
    setSetupWizardPage(2);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("mod.io API Access"), {
      target: { value: "key" },
    });

    await expect(callTourAction("setup:save-modio")).resolves.toBe(false);

    expect(
      await screen.findByText(/Please enter your mod.io personal access token/),
    ).not.toBeNull();
  });

  it("logs out when the mod.io token is refused", async () => {
    mocks.validateAndSaveModioToken.mockResolvedValueOnce(false);
    setSetupWizardPage(2);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("mod.io API Access"), {
      target: { value: "key" },
    });
    await fireEvent.input(
      screen.getByLabelText("mod.io Personal Access Token"),
      { target: { value: "token" } },
    );

    await expect(callTourAction("setup:save-modio")).resolves.toBe(false);

    expect(mocks.logout).toHaveBeenCalledTimes(1);
    expect(mocks.tokenSet).toHaveBeenCalledWith(false);
    expect(
      await screen.findByText(/Personal access token is invalid/),
    ).not.toBeNull();
  });

  it("reports when mod.io validation throws", async () => {
    mocks.validateAndSaveModioApiKey.mockRejectedValueOnce(new Error("down"));
    setSetupWizardPage(2);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("mod.io API Access"), {
      target: { value: "key" },
    });
    await fireEvent.input(
      screen.getByLabelText("mod.io Personal Access Token"),
      { target: { value: "token" } },
    );

    await expect(callTourAction("setup:save-modio")).resolves.toBe(false);

    expect(await screen.findByText(/Failed to validate/)).not.toBeNull();
  });

  it("clears a mod.io error once the user types again", async () => {
    setSetupWizardPage(2);
    render(SetupWizard);
    await callTourAction("setup:save-modio");
    expect(await screen.findByText(/Please enter your/)).not.toBeNull();

    await fireEvent.input(screen.getByLabelText("mod.io API Access"), {
      target: { value: "k" },
    });

    expect(screen.queryByText(/Please enter your/)).toBeNull();
  });

  it("reports an invalid Nexus key", async () => {
    mocks.validateAndSaveNexusApiKey.mockResolvedValueOnce(false);
    setSetupWizardPage(3);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Nexus Mods API Key"), {
      target: { value: "bad" },
    });

    await expect(callTourAction("setup:save-nexus")).resolves.toBe(false);

    expect(await screen.findByText(/Invalid Nexus API key/)).not.toBeNull();
  });

  it("reports when Nexus validation throws", async () => {
    mocks.validateAndSaveNexusApiKey.mockRejectedValueOnce(new Error("down"));
    setSetupWizardPage(3);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Nexus Mods API Key"), {
      target: { value: "key" },
    });

    await expect(callTourAction("setup:save-nexus")).resolves.toBe(false);

    expect(await screen.findByText(/Failed to validate/)).not.toBeNull();
  });

  it("finishes setup when the modpack URL is empty", async () => {
    setSetupWizardPage(4);
    render(SetupWizard);

    await expect(callTourAction("setup:finish")).resolves.toBe(true);

    expect(mocks.fetchModpackJson).not.toHaveBeenCalled();
    expect(mocks.updateConfig).toHaveBeenCalledWith({
      setup_wizard_complete: true,
    });
  });

  it("rejects a modpack URL without a scheme", async () => {
    setSetupWizardPage(4);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Modpack URL"), {
      target: { value: "example.com/modpack.json" },
    });

    await expect(callTourAction("setup:finish")).resolves.toBe(false);

    expect(await screen.findByText(/should start with http/)).not.toBeNull();
  });

  it("reports when saving the modpack URL fails", async () => {
    mocks.setModpackUrl.mockRejectedValueOnce(new Error("disk full"));
    setSetupWizardPage(4);
    render(SetupWizard);
    await fireEvent.input(screen.getByLabelText("Modpack URL"), {
      target: { value: "https://example.com/modpack.json" },
    });

    await expect(callTourAction("setup:finish")).resolves.toBe(false);

    expect(await screen.findByText(/Failed to save/)).not.toBeNull();
  });

  it("opens the mod.io pages without failing when the opener is broken", async () => {
    setSetupWizardPage(2);
    render(SetupWizard);
    mocks.openUrl.mockRejectedValueOnce(new Error("no browser"));

    await fireEvent.click(screen.getByText("Open mod.io API Access Page"));
    await fireEvent.click(screen.getByText("Open Personal Access Tokens Page"));

    await vi.waitFor(() => {
      expect(mocks.openUrl).toHaveBeenCalledWith("https://mod.io/me/access");
    });
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://mod.io/me/access#tokens",
    );
  });

  it("opens the Nexus page", async () => {
    setSetupWizardPage(3);
    render(SetupWizard);

    await fireEvent.click(screen.getByText("Open Nexus API Keys Page"));

    await vi.waitFor(() => {
      expect(mocks.openUrl).toHaveBeenCalledWith(
        "https://www.nexusmods.com/settings/api-keys",
      );
    });
  });

  it("toggles the mod.io secrets between hidden and visible", async () => {
    setSetupWizardPage(2);
    render(SetupWizard);

    const key = screen.getByLabelText("mod.io API Access") as HTMLInputElement;
    const token = screen.getByLabelText(
      "mod.io Personal Access Token",
    ) as HTMLInputElement;
    expect(key.type).toBe("password");
    expect(token.type).toBe("password");

    await fireEvent.click(screen.getByTitle("Show key"));
    await fireEvent.click(screen.getByTitle("Show token"));

    expect(key.type).toBe("text");
    expect(token.type).toBe("text");

    await fireEvent.click(screen.getByTitle("Hide key"));
    await fireEvent.click(screen.getByTitle("Hide token"));

    expect(key.type).toBe("password");
    expect(token.type).toBe("password");
  });

  it("toggles the Nexus secret between hidden and visible", async () => {
    setSetupWizardPage(3);
    render(SetupWizard);

    const key = screen.getByLabelText("Nexus Mods API Key") as HTMLInputElement;
    expect(key.type).toBe("password");

    await fireEvent.click(screen.getByTitle("Show key"));
    expect(key.type).toBe("text");

    await fireEvent.click(screen.getByTitle("Hide key"));
    expect(key.type).toBe("password");
  });

  it("clears the game path error once the user types again", async () => {
    render(SetupWizard);
    await fireEvent.click(screen.getByText("Auto Detect"));
    expect(await screen.findByText(/Could not auto-detect/)).not.toBeNull();

    await fireEvent.input(screen.getByLabelText("Game path"), {
      target: { value: "x" },
    });

    expect(screen.queryByText(/Could not auto-detect/)).toBeNull();
  });
});
