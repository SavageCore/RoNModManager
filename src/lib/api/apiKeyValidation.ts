import {
  saveToken,
  updateConfig,
  validateToken,
  verifyModioApiKey,
  verifyNexusApiKey,
} from "$lib/api/commands";
import { tokenStore } from "$lib/stores/token";

export async function validateAndSaveModioApiKey(
  key: string,
): Promise<boolean> {
  const valid = await verifyModioApiKey(key);
  if (!valid) return false;
  await updateConfig({ modio_api_key: key });
  return true;
}

export async function validateAndSaveModioToken(
  token: string,
): Promise<boolean> {
  await saveToken(token);
  const valid = await validateToken();
  if (valid) tokenStore.set(true);
  return valid;
}

export async function validateAndSaveNexusApiKey(
  key: string,
): Promise<boolean> {
  const valid = await verifyNexusApiKey(key);
  if (!valid) return false;
  await updateConfig({ nexus_api_key: key });
  return true;
}
