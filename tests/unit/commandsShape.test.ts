import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import {
  getConfig,
  installLocalMod,
  syncModpackToRemote,
  updateConfig,
  verifyModioApiKey,
  verifyNexusApiKey,
} from "../../src/lib/api/commands";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("command wrappers", () => {
  it("forwards get_config without args", async () => {
    invoke.mockResolvedValue({});
    await getConfig();
    expect(invoke).toHaveBeenCalledWith("get_config");
  });

  it("coerces optional install args to null", async () => {
    invoke.mockResolvedValue({ wasDuplicate: false });
    await installLocalMod("/tmp/mod.zip");
    expect(invoke).toHaveBeenCalledWith("install_local_mod", {
      filePath: "/tmp/mod.zip",
      selectedPakFiles: null,
      precomputedHash: null,
    });
  });

  it("passes explicit install args through", async () => {
    invoke.mockResolvedValue({ wasDuplicate: true });
    await installLocalMod("/tmp/mod.zip", ["a.pak"], "abc");
    expect(invoke).toHaveBeenCalledWith("install_local_mod", {
      filePath: "/tmp/mod.zip",
      selectedPakFiles: ["a.pak"],
      precomputedHash: "abc",
    });
  });

  it("defaults sync auth to Auto and non-verbose", async () => {
    invoke.mockResolvedValue(undefined);
    await syncModpackToRemote();
    expect(invoke).toHaveBeenCalledWith("sync_modpack_to_remote", {
      auth: { type: "Auto" },
      verbose: false,
    });
  });

  it("wraps config updates", async () => {
    invoke.mockResolvedValue(undefined);
    await updateConfig({ nexus_api_key: "k" });
    expect(invoke).toHaveBeenCalledWith("update_config", {
      updates: { nexus_api_key: "k" },
    });
  });

  it("verifies api keys with the key payload", async () => {
    invoke.mockResolvedValue(true);
    await verifyNexusApiKey("n");
    expect(invoke).toHaveBeenCalledWith("verify_nexus_api_key", {
      apiKey: "n",
    });
    await verifyModioApiKey("m");
    expect(invoke).toHaveBeenCalledWith("verify_modio_api_key", {
      apiKey: "m",
    });
  });
});
