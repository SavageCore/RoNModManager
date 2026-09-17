import { addModTourCommand } from "$lib/stores/tourUi";
import type { TourContext, TourStep } from "./types";

const EXAMPLE_MOD_URL =
  "https://mod.io/g/readyornot/m/uon-official#description";

const ADD_MOD_PANEL = '[data-tour="addmod-panel"]';
const PROFILE_SAVE = '[data-tour="profile-save"]';
const EXPORT_PANEL = '[data-tour="export-panel"]';
const IMPORT_LOG_PANEL = '[data-tour="import-log-panel"]';
const SYNC_AUTH_PANEL = '[data-tour="sync-auth-panel"]';

/// A sidebar step gives the user time to see the item light up, but does not
/// rush them the way a two second wait did.
const NAV_PAUSE_MS = 30000;

const firstRow = () =>
  document.querySelector('[data-tour="mod-row"]') as Element | null;

const selectorPresent = (
  ctx: TourContext,
  selector: string,
  timeoutMs: number,
) => ctx.waitForSelector(selector, timeoutMs).then(Boolean);

const selectorGone = (ctx: TourContext, selector: string, timeoutMs: number) =>
  ctx.waitFor(() => !document.querySelector(selector), timeoutMs).then(Boolean);

/// Steps inside the Add Mod dialog must work in both directions: entering one
/// from the front finds the dialog open, entering it with Back finds it closed
/// (because the step we came back from tidied up after itself).
const ensureAddModOpen = async (ctx: TourContext): Promise<void> => {
  if (!document.querySelector(ADD_MOD_PANEL)) {
    await ctx.call("mods:open-add-mod");
  }
};

export const TOUR_STEPS: TourStep[] = [
  {
    id: "welcome",
    // Replaying from Settings lands here, so the tour always starts on Mods.
    route: "/mods",
    title: "Welcome to RoN Mod Manager",
    body: "This short tour shows how to add mods, label and group them, switch profiles, and set up the options most people change.",
    bullets: [
      "Use the arrow keys as well as Back and Next.",
      "Skip tour stops it for good. You can replay it later from Settings > Tutorial.",
    ],
  },
  {
    id: "add-mod",
    route: "/mods",
    target: '[data-tour="mods-add-mod"]',
    title: "Add Mod",
    body: "Click Add Mod, or press Next, to open the mod installer. You can paste mod.io or Nexus Mods links, or install a file you already downloaded.",
    pulseTarget: true,
    nextAction: "mods:open-add-mod",
    // Stepping back here has to put the dialog away again, or it covers the
    // button this card is about.
    enter: async (ctx) => {
      await ctx.call("mods:close-add-mod");
    },
    ready: (ctx) => selectorPresent(ctx, ADD_MOD_PANEL, 60000),
    advanceOnReady: true,
  },
  {
    id: "add-mod-link",
    route: "/mods",
    placement: "right",
    target: '[data-tour="addmod-tab-link"]',
    title: "Add by link",
    pulseTarget: true,
    body: "Paste mod.io or Nexus Mods links, one per line. They install one after another.",
    bullets: [
      "mod.io links look like mod.io/g/readyornot/m/...",
      "Nexus links look like nexusmods.com/readyornot/mods/..., and open in your browser unless your account is premium.",
    ],
    enter: async (ctx) => {
      await ensureAddModOpen(ctx);
      addModTourCommand.set({ tab: "link", link: EXAMPLE_MOD_URL });
    },
  },
  {
    id: "add-mod-url",
    route: "/mods",
    placement: "right",
    target: '[data-tour="addmod-link-input"]',
    title: "Example link filled in",
    body: "We filled in Uncensored or Not from mod.io as an example. The same box also takes just the slug, such as uon-official, or the numeric mod ID from a mod's page.",
    enter: async (ctx) => {
      await ensureAddModOpen(ctx);
      addModTourCommand.set({ tab: "link" });
    },
  },
  {
    id: "add-mod-file",
    route: "/mods",
    placement: "right",
    target: '[data-tour="addmod-drop-zone"]',
    title: "Local File",
    body: "The Local File tab installs a .pak, .zip, .rar or .7z you already have. Click the box to browse, or drop the file here or anywhere on the Mods page.",
    enter: async (ctx) => {
      await ensureAddModOpen(ctx);
      addModTourCommand.set({ tab: "file" });
    },
  },
  {
    id: "add-mod-submit",
    route: "/mods",
    placement: "left",
    target: '[data-tour="addmod-submit"]',
    title: "Install it",
    body: "Press Add Mod or Next to install the example. It downloads in the background, so the dialog closes straight away. If it fails, check Settings > Accounts and API Keys for your mod.io credentials.",
    pulseTarget: true,
    nextAction: "mods:submit-add-mod",
    enter: async (ctx) => {
      await ensureAddModOpen(ctx);
      addModTourCommand.set({ tab: "link" });
      // Coming back from the log cards: the panel belongs to those, so put it
      // away and leave this card looking the way it did the first time.
      await ctx.call("shell:close-import-log");
    },
    ready: (ctx) => selectorGone(ctx, ADD_MOD_PANEL, 300000),
    advanceOnReady: true,
    leave: async (ctx) => {
      await ctx.call("mods:close-add-mod");
    },
  },
  {
    id: "import-log",
    route: "/mods",
    placement: "left",
    target: '[data-tour="import-log-entry"]',
    targetTimeoutMs: 60000,
    title: "Import Log",
    body: "Downloads and installs show up here, one entry per mod. Click an entry to expand it and see what happened.",
    enter: async (ctx) => {
      // Back from the next card has to bring the log back, and the dialog it
      // came from must be out of the way.
      await ctx.call("mods:close-add-mod");
      await ctx.call("shell:open-import-log");
    },
  },
  {
    id: "import-log-close",
    route: "/mods",
    placement: "left",
    target: '[data-tour="import-log-close"]',
    title: "Clear or close it",
    body: "Close, or Next, empties the list and hides the panel. Your mods are untouched.",
    bullets: ["Clear empties the entries but leaves the panel open."],
    pulseTarget: true,
    nextAction: "shell:close-import-log",
    enter: async (ctx) => {
      await ctx.call("shell:open-import-log");
    },
    ready: (ctx) => selectorGone(ctx, IMPORT_LOG_PANEL, 300000),
    advanceOnReady: true,
    leave: async (ctx) => {
      await ctx.call("shell:close-import-log");
    },
  },
  {
    id: "mod-row",
    route: "/mods",
    target: '[data-tour="mod-row"]',
    resolveTarget: firstRow,
    targetTimeoutMs: 240000,
    title: "Your mods",
    body: "That is your mod, downloaded into staging. Everything you add appears here.",
    ready: (ctx) => ctx.waitFor(firstRow, 240000).then(Boolean),
  },
  {
    id: "mod-row-toggle",
    route: "/mods",
    target: '[data-tour="mod-row-toggle"]',
    title: "Enable or disable",
    pulseTarget: true,
    body: "The switch enables this mod for the active profile, or disables it. The files stay in staging either way.",
    bullets: [
      "At rest, with this app closed, the game folder depends on Settings > Link mods only on launch:",
      "On: it stays stock, and only a Launch from this app links the mods.",
      "Off: the mods stay linked, so launching from Steam directly is modded too.",
    ],
  },
  {
    id: "mod-row-delete",
    route: "/mods",
    target: '[data-tour="mod-row-delete"]',
    title: "Remove a mod",
    pulseTarget: true,
    body: "The bin button removes the mod and its files from staging, after a confirmation, and the game folder is updated to match.",
  },
  {
    id: "mod-row-menu",
    route: "/mods",
    target: '[data-tour="mod-row"]',
    title: "Right-click a mod",
    body: "Everything that does not fit on the row lives on the right-click menu.",
    bullets: [
      "Refresh metadata - re-reads the name, version and status from the mod's page.",
      "Manage tags - your own labels, such as Visuals or Audio, to filter the list by.",
      "Manage collections - groups you switch together, such as an LSPD set. They travel with an exported modpack.",
      "Manage add-ons - extra .pak or world data files that belong to a mod.",
      "Mark as broken - flags a mod that stops the game or misbehaves.",
      "Edit link - points the mod at a different mod.io or Nexus page.",
    ],
  },
  {
    id: "header-profile",
    target: '[data-tour="header-profile"]',
    title: "Profiles",
    pulseTarget: true,
    body: "A profile is a saved set of mods, tags and collections. This dropdown switches between them from anywhere in the app.",
  },
  {
    id: "nav-profiles",
    target: '[data-tour="nav-profiles"]',
    autoAdvanceMs: NAV_PAUSE_MS,
    title: "Profiles page",
    body: "Profiles have their own page in the sidebar.",
  },
  {
    id: "profiles-create",
    route: "/profiles",
    target: '[data-tour="profiles-create"]',
    title: "Create a profile",
    body: "Profiles share one mod store, so a new one does not duplicate any files. Click Create New Profile, or press Next.",
    pulseTarget: true,
    nextAction: "profiles:open-create-form",
    // Coming back here means the form has to go, or the button this card is
    // about is hidden behind it.
    enter: async (ctx) => {
      await ctx.call("profiles:close-create-form");
    },
    ready: (ctx) => selectorPresent(ctx, PROFILE_SAVE, 60000),
    advanceOnReady: true,
  },
  {
    id: "profile-name",
    route: "/profiles",
    placement: "left",
    target: '[data-tour="profile-name"]',
    title: "Profile name",
    body: "Give the profile a name.",
    enter: async (ctx) => {
      await ctx.call("profiles:ensure-create-form");
    },
  },
  {
    id: "profile-description",
    route: "/profiles",
    placement: "left",
    target: '[data-tour="profile-description"]',
    title: "Description",
    body: "Optional, and worth filling in if you keep several profiles.",
    enter: async (ctx) => {
      await ctx.call("profiles:ensure-create-form");
    },
  },
  {
    id: "profile-actions",
    route: "/profiles",
    placement: "left",
    target: '[data-tour="profile-actions"]',
    title: "Save or cancel",
    body: "Save creates the profile and makes it active straight away. Cancel backs out without saving anything.",
    bullets: [
      "You can just cancel for now and we will carry on.",
      "Next does the same job: it cancels unless the name or description has text in it, then it saves.",
    ],
    pulseTarget: true,
    nextAction: "profiles:submit-create-form",
    enter: async (ctx) => {
      await ctx.call("profiles:ensure-create-form");
    },
    ready: (ctx) => selectorGone(ctx, PROFILE_SAVE, 120000),
    advanceOnReady: true,
    leave: async (ctx) => {
      await ctx.call("profiles:close-create-form");
    },
  },
  {
    id: "profile-apply",
    route: "/profiles",
    target: '[data-tour="profile-apply"]',
    title: "Apply",
    pulseTarget: true,
    body: "Apply the profile. An alternative to the menu bar drop-down.",
    enter: async (ctx) => {
      // The profile list is what this card is about, so the form goes away.
      await ctx.call("profiles:close-create-form");
    },
  },
  {
    id: "header-refresh",
    route: "/mods",
    target: '[data-tour="header-refresh"]',
    title: "Refresh",
    pulseTarget: true,
    body: "This icon re-reads the details for every mod that has a source link, and checks mod.io and Nexus for updates. A mod with a newer version gets an orange update pill next to it - click the pill to install the update.",
  },
  {
    id: "nav-settings",
    target: '[data-tour="nav-settings"]',
    autoAdvanceMs: NAV_PAUSE_MS,
    title: "Settings",
    body: "Settings has its own page in the sidebar.",
  },
  {
    id: "settings-theme",
    route: "/settings",
    target: '[data-tour="settings-theme"]',
    title: "Theme",
    body: "Theme follows your desktop by default, or you can pin light or dark. Watch the whole window change - the tour switches it and puts it back after a moment, then press Next when you are ready.",
    enter: async (ctx) => {
      await ctx.call("settings:demo-theme");
    },
    // Leaving the card (either way) puts the theme back and drops the timer.
    leave: async (ctx) => {
      await ctx.call("settings:stop-theme-demo");
    },
    // Deliberately no auto-advance: the demo reverts on its own and the flashing
    // Next is the cue, so a slow reader is never rushed off the card.
  },
  {
    id: "settings-link-on-launch",
    route: "/settings",
    target: '[data-tour="settings-link-on-launch"]',
    title: "Link mods only on launch",
    body: "The description under this setting explains when the game folder is updated.",
  },
  {
    id: "settings-intro-skip",
    route: "/settings",
    target: '[data-tour="settings-intro-skip"]',
    title: "Game Tweaks: Intro Skip",
    body: "Removes the startup movies. It renames the files in place, so turning it back off restores them.",
  },
  {
    id: "settings-optimization",
    route: "/settings",
    target: '[data-tour="settings-optimization"]',
    title: "Game Tweaks: Engine.ini optimisation",
    body: "Pick your GPU and press Apply to write a tuned Engine.ini from UE5 Performance Overhaul by AlexRenderX. Restore puts the original back.",
  },
  {
    id: "settings-sync-export",
    route: "/settings",
    target: '[data-tour="settings-sync-export"]',
    title: "Modpacks",
    body: "A modpack is a modpack.json manifest plus a mods folder. Export writes both, then you can host them and share a single link.",
  },
  {
    id: "settings-export-modal",
    route: "/settings",
    target: '[data-tour="settings-export-button"]',
    title: "Export first",
    body: "Press Export or Next to open the export dialog. Exporting always comes first, because syncing uploads whatever you exported last.",
    pulseTarget: true,
    nextAction: "settings:open-export-modal",
    ready: (ctx) => selectorPresent(ctx, EXPORT_PANEL, 60000),
    advanceOnReady: true,
  },
  {
    id: "settings-export-dialog",
    route: "/settings",
    placement: "left",
    // The whole modal, so the Hosting guide link in its header sits inside the
    // highlight rather than the form fields alone.
    target: '[data-tour="export-modal-panel"]',
    title: "Export dialog",
    body: "Fill in the details and Export writes modpack.json plus the mods folder into your Downloads.",
    bullets: [
      "Hosting guide opens the README notes on self-hosting and the ronmm:// one-click link.",
      "Press Next to close the dialog for now - nothing is exported until you press Export.",
    ],
    enter: async (ctx) => {
      await ctx.call("settings:ensure-export-modal");
    },
    leave: async (ctx) => {
      await ctx.call("settings:close-export-modal");
    },
  },
  {
    id: "settings-sync-credentials",
    route: "/settings",
    target: SYNC_AUTH_PANEL,
    targetTimeoutMs: 60000,
    title: "Credentials",
    body: "This is the dialog Sync Now opens when your system keys are not enough. Choose a password or a key file, then Test to check it against the server.",
    bullets: [
      "Nothing is written to your config file - the password or key passphrase is used for that sync only.",
    ],
    enter: async (ctx) => {
      await ctx.call("settings:open-sync-credentials");
    },
    leave: async (ctx) => {
      await ctx.call("settings:close-sync-credentials");
    },
  },
  {
    id: "settings-sync-now",
    route: "/settings",
    target: '[data-tour="settings-sync-now"]',
    title: "Sync to a server",
    body: "Sync Now uploads the modpack.json and the mods folder to the host and path above, over SFTP, using the credentials above if it needs them.",
  },
  {
    id: "settings-tutorial",
    // Back from the closing card has to bring the user back to Settings.
    route: "/settings",
    target: '[data-tour="settings-tutorial"]',
    title: "Replay the tour",
    pulseTarget: true,
    body: "Replay Tutorial starts this tour again from the beginning.",
  },
  {
    id: "done",
    route: "/mods",
    title: "That's a wrap!",
    body: "Press Finish to continue using the app.",
  },
];
