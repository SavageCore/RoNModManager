import { fireEvent, render, screen } from "@testing-library/svelte";
import { get } from "svelte/store";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  updateConfig: vi.fn(async () => undefined),
  getConfig: vi.fn(async () => ({ game_path: "", modpack_url: "" })),
  setGamePath: vi.fn(async () => undefined),
  setModpackUrl: vi.fn(async () => undefined),
  fetchModpackJson: vi.fn(async () => ({ version: "1.0.0" })),
  detectGamePath: vi.fn(async () => null),
  logout: vi.fn(async () => undefined),
  validateAndSaveModioApiKey: vi.fn(async () => true),
  validateAndSaveModioToken: vi.fn(async () => true),
  validateAndSaveNexusApiKey: vi.fn(async () => true),
  openDialog: vi.fn(async () => null),
  openUrl: vi.fn(async () => undefined),
  toastSuccess: vi.fn(),
  tokenSet: vi.fn(),
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
  toastStore: { success: mocks.toastSuccess, error: vi.fn() },
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
});
