import { writable } from "svelte/store";

export const pendingInstallUrl = writable<string | null>(null);
