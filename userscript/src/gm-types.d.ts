// Type declarations for Tampermonkey / Greasemonkey legacy APIs (GM_*)
// that @types/greasemonkey doesn't cover. These are the callback-based
// global functions used by Tampermonkey.

declare interface GM_XHRResponse {
  readonly response: any;
  readonly responseText: string;
  readonly responseHeaders: string;
  readonly status: number;
  readonly statusText: string;
  readonly readyState: number;
  readonly finalUrl: string;
  readonly context?: unknown;
  readonly responseXML: Document | false;
  readonly lengthComputable: boolean;
  readonly loaded: number;
  readonly total: number;
  readonly timeout: number;
}

declare interface GM_XHRRequest {
  method?: string;
  url: string;
  headers?: Record<string, string>;
  data?: string | Blob | ArrayBuffer;
  responseType?: XMLHttpRequestResponseType;
  timeout?: number;
  context?: unknown;
  onload?: (response: GM_XHRResponse) => void;
  onerror?: (response: GM_XHRResponse) => void;
  ontimeout?: (response: GM_XHRResponse) => void;
  onprogress?: (response: GM_XHRResponse) => void;
  onloadstart?: (response: GM_XHRResponse) => void;
  onloadend?: (response: GM_XHRResponse) => void;
  onabort?: (response: GM_XHRResponse) => void;
}

/** Legacy Tampermonkey GM_xmlhttpRequest (callback-based). */
declare function GM_xmlhttpRequest(
  details: GM_XHRRequest,
  onload?: (response: GM_XHRResponse) => void,
): void;

declare function GM_getValue(key: string, defaultValue?: unknown): unknown;
declare function GM_setValue(key: string, value: unknown): void;
declare function GM_deleteValue(key: string): void;
declare function GM_listValues(): string[];
declare function GM_registerMenuCommand(
  name: string,
  functionReference: () => void,
  accessKey?: string,
): void;
