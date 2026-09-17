import { writable } from "svelte/store";

export type AddModTourCommand = { tab: "link" | "file"; link?: string };

/// One-shot command the tour leaves for AddModModal to consume. A prop would
/// be overwritten by reactivity every time the user switches tab themselves.
export const addModTourCommand = writable<AddModTourCommand | null>(null);
