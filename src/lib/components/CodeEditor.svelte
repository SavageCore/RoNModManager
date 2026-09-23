<script lang="ts">
  // Raw-text code editor with lightweight syntax highlighting, shared by the
  // config.json and config.lua tabs. Zero dependencies: a transparent
  // textarea sits over a highlighted <pre> backdrop with identical metrics,
  // and scroll positions stay synced. Highlighting is cosmetic only - the
  // textarea always holds the real text.
  export let value: string = "";
  export let language: "json" | "lua" = "json";
  export let editorId: string;
  export let rows: number = 20;

  let textarea: HTMLTextAreaElement | null = null;
  let backdrop: HTMLElement | null = null;

  function esc(chunk: string): string {
    return chunk
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  const JSON_TOKEN =
    /("(?:[^"\\\n]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|\b(true|false|null)\b|([{}\[\],:])/g;

  const LUA_TOKEN =
    /(--\[\[[\s\S]*?\]\])|(--[^\n]*)|("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|\[\[[\s\S]*?\]\])|\b(0[xX][0-9a-fA-F]+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)\b|\b(local|function|end|if|then|else|elseif|for|while|do|break|return|repeat|until|in|and|or|not|nil|true|false)\b/g;

  function highlightJson(src: string): string {
    let out = "";
    let last = 0;
    JSON_TOKEN.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = JSON_TOKEN.exec(src)) !== null) {
      out += esc(src.slice(last, m.index));
      last = m.index + m[0].length;
      if (m[1] !== undefined) {
        out += `<span class="tok-key">${esc(m[1])}</span>${esc(m[2] ?? "")}`;
      } else if (m[3] !== undefined) {
        out += `<span class="tok-num">${esc(m[3])}</span>`;
      } else if (m[4] !== undefined) {
        out += `<span class="tok-kw">${esc(m[4])}</span>`;
      } else {
        out += `<span class="tok-punct">${esc(m[0])}</span>`;
      }
    }
    return out + esc(src.slice(last));
  }

  function highlightLua(src: string): string {
    let out = "";
    let last = 0;
    LUA_TOKEN.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = LUA_TOKEN.exec(src)) !== null) {
      out += esc(src.slice(last, m.index));
      last = m.index + m[0].length;
      if (m[1] !== undefined || m[2] !== undefined) {
        out += `<span class="tok-comment">${esc(m[0])}</span>`;
      } else if (m[3] !== undefined) {
        out += `<span class="tok-str">${esc(m[0])}</span>`;
      } else if (m[4] !== undefined) {
        out += `<span class="tok-num">${esc(m[0])}</span>`;
      } else {
        out += `<span class="tok-kw">${esc(m[0])}</span>`;
      }
    }
    return out + esc(src.slice(last));
  }

  function highlightCode(src: string, lang: "json" | "lua"): string {
    const body = lang === "json" ? highlightJson(src) : highlightLua(src);
    // Trailing newline so the backdrop matches the textarea's final line.
    return body + "\n";
  }

  function syncScroll() {
    if (textarea && backdrop) {
      backdrop.scrollTop = textarea.scrollTop;
      backdrop.scrollLeft = textarea.scrollLeft;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Tab inserts two spaces instead of leaving the editor.
    if (e.key === "Tab") {
      e.preventDefault();
      const el = e.currentTarget as HTMLTextAreaElement;
      const start = el.selectionStart ?? value.length;
      const end = el.selectionEnd ?? value.length;
      value = value.slice(0, start) + "  " + value.slice(end);
      queueMicrotask(() => {
        el.selectionStart = el.selectionEnd = start + 2;
      });
    }
  }

  $: code = highlightCode(value ?? "", language);
</script>

<div class="code-editor">
  <pre aria-hidden="true" bind:this={backdrop}><code>{@html code}</code></pre>
  <textarea
    id={editorId}
    bind:value
    bind:this={textarea}
    spellcheck={false}
    wrap="off"
    {rows}
    autocomplete="off"
    autocapitalize="off"
    on:scroll={syncScroll}
    on:keydown={onKeydown}></textarea>
</div>

<style>
  .code-editor {
    position: relative;
    border: 1px solid var(--adw-border-color);
    background: var(--clr-surface);
  }
  .code-editor pre,
  .code-editor textarea {
    margin: 0;
    padding: 6px 8px;
    font-family: ui-monospace, monospace;
    font-size: 12px;
    line-height: 1.5;
    tab-size: 2;
    white-space: pre;
    overflow-wrap: normal;
    word-break: normal;
  }
  .code-editor pre {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }
  .code-editor pre code {
    color: var(--clr-text);
  }
  .code-editor textarea {
    position: relative;
    display: block;
    width: 100%;
    box-sizing: border-box;
    background: transparent;
    border: none;
    outline: none;
    resize: vertical;
    overflow: auto;
    color: transparent;
    caret-color: var(--clr-text);
  }
  .code-editor textarea::selection {
    background: var(--clr-primary-300);
  }
  .code-editor :global(.tok-key) {
    color: var(--clr-primary-300);
  }
  .code-editor :global(.tok-str) {
    color: var(--clr-success-300, #9ece6a);
  }
  .code-editor :global(.tok-num) {
    color: var(--clr-warning-300, #e0af68);
  }
  .code-editor :global(.tok-kw) {
    color: var(--clr-primary-300);
  }
  .code-editor :global(.tok-comment) {
    color: var(--clr-text-secondary);
    font-style: italic;
  }
  .code-editor :global(.tok-punct) {
    color: var(--clr-text-secondary);
  }
</style>
