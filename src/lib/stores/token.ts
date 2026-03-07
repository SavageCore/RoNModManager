import { writable } from "svelte/store";

export const tokenStore = writable<boolean>(false);
