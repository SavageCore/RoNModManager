import { beforeEach, describe, expect, it, vi } from "vitest";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    saveToken: vi.fn<(token: string) => Promise<void>>(),
    updateConfig: vi.fn<(patch: Record<string, unknown>) => Promise<void>>(),
    validateToken: vi.fn<(token: string) => Promise<boolean>>(),
    verifyModioApiKey: vi.fn<(key: string) => Promise<boolean>>(),
    verifyNexusApiKey: vi.fn<(key: string) => Promise<boolean>>(),
  },
}));

vi.mock("../../src/lib/api/commands", () => mocks);

import {
  validateAndSaveModioApiKey,
  validateAndSaveModioToken,
  validateAndSaveNexusApiKey,
} from "../../src/lib/api/apiKeyValidation";
import { tokenStore } from "../../src/lib/stores/token";
import { get } from "svelte/store";

beforeEach(() => {
  vi.clearAllMocks();
  tokenStore.set(false);
});

describe("validateAndSaveModioApiKey", () => {
  it("saves the key when valid", async () => {
    mocks.verifyModioApiKey.mockResolvedValue(true);
    await expect(validateAndSaveModioApiKey("k")).resolves.toBe(true);
    expect(mocks.updateConfig).toHaveBeenCalledWith({ modio_api_key: "k" });
  });

  it("rejects without saving when invalid", async () => {
    mocks.verifyModioApiKey.mockResolvedValue(false);
    await expect(validateAndSaveModioApiKey("bad")).resolves.toBe(false);
    expect(mocks.updateConfig).not.toHaveBeenCalled();
  });
});

describe("validateAndSaveNexusApiKey", () => {
  it("saves the key when valid", async () => {
    mocks.verifyNexusApiKey.mockResolvedValue(true);
    await expect(validateAndSaveNexusApiKey("k")).resolves.toBe(true);
    expect(mocks.updateConfig).toHaveBeenCalledWith({ nexus_api_key: "k" });
  });

  it("rejects without saving when invalid", async () => {
    mocks.verifyNexusApiKey.mockResolvedValue(false);
    await expect(validateAndSaveNexusApiKey("bad")).resolves.toBe(false);
    expect(mocks.updateConfig).not.toHaveBeenCalled();
  });
});

describe("validateAndSaveModioToken", () => {
  it("saves, validates and flags the token store", async () => {
    mocks.validateToken.mockResolvedValue(true);
    await expect(validateAndSaveModioToken("t")).resolves.toBe(true);
    expect(mocks.saveToken).toHaveBeenCalledWith("t");
    expect(get(tokenStore)).toBe(true);
  });

  it("does not flag the store when invalid", async () => {
    mocks.validateToken.mockResolvedValue(false);
    await expect(validateAndSaveModioToken("t")).resolves.toBe(false);
    expect(mocks.saveToken).toHaveBeenCalledWith("t");
    expect(get(tokenStore)).toBe(false);
  });
});
