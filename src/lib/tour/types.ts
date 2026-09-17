export type TourRoute = "/mods" | "/profiles" | "/settings";

export type TourActionName = keyof TourActions;

export type TourActions = {
  "mods:open-add-mod": () => void;
  "mods:close-add-mod": () => void;
  "mods:submit-add-mod": () => void;
  "profiles:open-create-form": () => void;
  "profiles:ensure-create-form": () => void;
  "profiles:submit-create-form": () => void;
  "profiles:close-create-form": () => void;
  "settings:close-export-modal": () => void;
  "settings:open-export-modal": () => void;
  "settings:ensure-export-modal": () => void;
  "settings:demo-theme": () => void;
  "settings:stop-theme-demo": () => void;
  "settings:open-sync-credentials": () => void;
  "settings:close-sync-credentials": () => void;
  "shell:close-import-log": () => void;
  "shell:open-import-log": () => void;
};

export type TourContext = {
  call: (name: TourActionName) => Promise<void>;
  goto: (route: TourRoute) => Promise<void>;
  waitFor: <T>(
    predicate: () => T | null | false | undefined,
    timeoutMs: number,
  ) => Promise<T | null>;
  waitForSelector: (
    selector: string,
    timeoutMs?: number,
  ) => Promise<Element | null>;
};

export type TourStep = {
  id: string;
  title: string;
  body?: string;
  /// Short lines rendered as a list under the body. Keeps a step from becoming
  /// a wall of text.
  bullets?: string[];
  /// Where the card sits relative to the target. "auto" places it below, or
  /// above when there is no room.
  placement?: "auto" | "left" | "right";
  route?: TourRoute;
  target?: string;
  /// Element used only to place the card when there is no target worth ringing.
  anchorTo?: string;
  /// Ring the target. Set false to cut the dim around it without the outline.
  showRing?: boolean;
  /// Make the ring flash instead of drawing a steady border. Used for controls
  /// the user presses, rather than things they only need to read.
  pulseTarget?: boolean;
  targetTimeoutMs?: number;
  resolveTarget?: () => Element | null;
  /// Advance on its own after this long. Used for page transitions, where the
  /// next step navigates.
  autoAdvanceMs?: number;
  /// Move on as soon as the step's ready condition is met, so the user does not
  /// have to click the app and then the tour's Next.
  advanceOnReady?: boolean;
  /// What the tour's own Next button should do. When set, Next is always
  /// enabled and performs the same action as the control being highlighted, so
  /// the user can either click the control or press Next.
  nextAction?: TourActionName;
  enter?: (ctx: TourContext) => Promise<void>;
  leave?: (ctx: TourContext) => Promise<void>;
  ready?: (ctx: TourContext) => Promise<boolean>;
};
