# Web source

- URL: https://theneuralbase.com/github-copilot/learn/intermediate/performance-issue-identification
- Title: Performance issue identification | Github Copilot Intermediate Course | The Neural Base
- Author(s): The Neural Base
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-05T10:27:44.322083296+00:00
- Relevance: High — title matches query


```text
ToolIntermediatemedium· 7 minbest_practiceIdentify when Copilot is slowing down your editor and diagnose root causes using VS Code's built-in performance tools.Verified April 2026 - the article's guidance on Timeline (Cmd+Shift+P on macOS, Ctrl+Shift+P on Windows/Linux) and Extension Performance Status (VS Code 1.95+) remains current. Critical production clarification: request latency in the Copilot output panel measures server roundtrip time only and does NOT include the time for the suggestion to render on screen - a completion showing 200ms latency may feel instant if local activation is <50ms, or feel sluggish if local activation hits 300ms due to other extensions.
The github.copilot.disabledLanguages array does NOT prevent activation on those languages alone - you must also set github.copilot.enable with perlanguage overrides (both together create the complete filtering). Common misdiagnosis: developers see 800ms latency and blame Copilot servers, but the real problem is often a single extension taking 400ms+ to activate per keystroke - check Timeline for the full picture before concluding the issue is networkside.
If using Copilot Free (claude3.5sonnet backend), completion latency is typically 15-20% faster for small files but slightly slower for large multiline completions compared to standard Copilot.Why this matters
Copilot can cause noticeable lag during autocomplete, especially in large files or on older hardware. Silent performance degradation reduces developer velocity and creates friction that teams often blame on the tool itself rather than misconfiguration. Identifying the real cause: extension conflicts, model loading, network latency, or aggressive completion settings: lets you fix the actual problem instead of disabling Copilot entirely.Skip if:If your editor is already fast and Copilot completions feel instant, skip aggressive tuning. If you're on a machine with <8GB RAM or a very old CPU, the cost-benefit of diagnosis may not justify the effort: consider disabling Copilot on that machine instead.ExplanationGitHub Copilot's performance depends on three factors: (1) the time it takes to generate completions on Anthropic's servers or your local model, (2) VS Code's extension system overhead, and (3) conflicts with other extensions. VS Code exposes performance metrics through the Developer: Open Timeline command and the Extension Performance Status view. These tools show which extensions are consuming CPU, memory, and disk I/O.
The most common cause of Copilot slowness is not Copilot itself: it's other extensions running activation events on every keystroke, or network requests blocking the UI thread. The second cause is aggressive completion settings (inline suggestions on every keystroke in large files). The third is an older Copilot cache or extension version.
To identify the root cause, use VS Code's built-in profiler to measure extension activation time, check the Copilot output panel for request latency, and disable other extensions one at a time to isolate conflicts. This is a 5-minute diagnosis that saves hours of frustration.Configuration{  "github.copilot.enable": {    "*": true,    "plaintext": false,    "markdown": false  },  "github.copilot.advanced": {    "debug.overrideCertVerification": false,    "debug.testOverrideProxyUrl": "",    "debug.useDeepSeek": false  },  "github.copilot.autocomplete.enable": true,  "editor.inlineSuggest.enabled": true,  "editor.inlineSuggest.suppressSuggestions": false,  "github.copilot.disabledLanguages": [    "plaintext",    "markdown",    "yaml"  ]}Why this order?
First, enable/disable Copilot per filetype to prevent unnecessary activation on files that don't benefit (markdown, plaintext, YAML). Second, configure Copilot-specific advanced options (most are debug flags and rarely needed). Third, configure VS Code's inline suggestion behavior at the editor level. Finally, disable Copilot on specific languages to prevent extension overhead on non-code files.Wrong vs RightTool vitalsPrimary commandDeveloper: Open Timeline (Cmd+Shift+P on macOS, Ctrl+Shift+P on Windows/Linux)Config file.vscode/settings.jsonVerifyView Output panel → select 'GitHub Copilot' from dropdown → check request latency logsIntegration notesCopilot conflicts with extensions that hook into VS Code's completion provider API (e.g., Tabnine, IntelliCode, PyLance in aggressive mode). If you use multiple completion extensions, only one should be set to 'enabled: true': the others should be set to 'disabled' or have their completion providers disabled in settings. Check the 'Extension Performance Status' view (available in VS Code 1.95+) to see CPU/memory consumption per extension.Migration pathIf Copilot remains slow after diagnosis, you can switch to Copilot Free (lower model, potentially faster for simple completions) or disable it entirely with 'github.copilot.enable: false'. You can also use @workspace context selectively (in Copilot Chat) rather than relying on autocomplete for every keystroke.Common gotcha
The GitHub Copilot output panel shows 'request latency' which includes network round-trip time, but VS Code's Timeline shows 'extension activation time' which is local overhead. A completion can show 800ms network latency (normal) but 50ms local activation time (good), making the user experience feel slow even though Copilot is working correctly. Always check both: if network latency > 3 seconds consistently, the issue is your network or API load, not your config. If local activation > 500ms, you have extension conflicts.Team adoption
In a team setting, commit a shared .vscode/settings.json with 'github.copilot.enable' filtering and disabled languages defined. In your team's onboarding docs, include: (1) run 'Developer: Open Timeline' to check baseline performance on day 1, (2) if > 1 second total, check the Extension Performance Status view, (3) if other extensions use completion providers, disable their completion features to avoid conflicts. This prevents 'Copilot is slow' discussions from becoming religious arguments: the data is visible to everyone.Most developers don't realize that disabling Copilot on plaintext, markdown, and YAML files eliminates 40-60% of extension activation events because these are common files you open frequently but rarely need completions in. Adding these three languages to 'github.copilot.disabledLanguages' is a one-line fix that feels like a 30% performance improvement. Additionally, if you're on VS Code 1.96+, the 'github.copilot.advanced.inlineCompletionLatencyThreshold' setting (in milliseconds) lets you only show completions if they arrive within X milliseconds: this prevents slow completions from blocking your editing.Why might a developer see 'Copilot request took 1.2 seconds' in the output panel but the Timeline shows 'GitHub Copilot extension activation: 80ms', and which one indicates the actual performance problem?
Show answer hintThe 1.2-second figure includes network latency to Anthropic's API (or your local model inference time), which is expected. The 80ms is local extension overhead. If local overhead is < 200ms, the slowness is network/model inference (expected, not a config issue). If local overhead is > 500ms, you have extension conflicts that tuning can fix./g,">").replace(/"/g,""");
}

function renderCard(c, voted) {
  const div = document.createElement("div");
  const isTeam = c.name === "theneuralbase_team";
  div.className = "cs-comment-card" + (isTeam ? " cs-team-card" : "");
  div.dataset.id = c.id;
  const hasVoted = voted.has(String(c.id));
  const codeId = `cs-code-${c.id}`;
  const codeBlock = c.code_snippet ? `
    ` : "";
  const teamBadge = isTeam
    ? `✓ verified`
    : (c.seeded ? `example note` : "");
  div.innerHTML = `
    
    ${codeBlock}
    `;
  // Copy button
  const copyBtn = div.querySelector(".cs-copy-btn");
  if (copyBtn) {
    copyBtn.addEventListener("click", () => {
      const target = document.getElementById(copyBtn.dataset.target);
      if (!target) return;
      navigator.clipboard.writeText(target.textContent).then(() => {
        copyBtn.textContent = "Copied!";
        copyBtn.classList.add("copied");
        setTimeout(() => { copyBtn.textContent = "Copy"; copyBtn.classList.remove("copied"); }, 2000);
      }).catch(() => {
        // fallback
        const sel = window.getSelection();
        const range = document.createRange();
        range.selectNodeContents(target);
        sel.removeAllRanges();
        sel.addRange(range);
      });
    });
  }

  div.querySelector(".cs-helpful-btn").addEventListener("click", async (e) => {
    const btn = e.currentTarget, id = btn.dataset.id;
    const vs = getVoted();
    if (vs.has(String(id))) return;
    btn.classList.add("voted");
    btn.querySelector(".helpful-count").textContent = parseInt(btn.querySelector(".helpful-count").textContent)+1;
    vs.add(String(id)); saveVoted(vs);
    await fetch(API,{method:"POST",headers:{"Content-Type":"application/json"},
      body:JSON.stringify({action:"helpful",id})}).catch(()=>{});
  });
  return div;
}

function renderSkeleton() {
  return ``;
}

async function loadComments() {
  const sidebarList  = document.querySelector(".cs-sidebar-list");
  const sidebarCount = document.querySelector(".cs-sidebar-count");
  const bottomList   = document.querySelector(".cs-bottom-list");
  const bottomCount  = document.querySelector(".cs-bottom-count");

  // If SSR rendered comments already, skip initial fetch — wire buttons and populate sidebar instead
  const hasSSR = bottomList && bottomList.querySelector(".cs-ssr-card");
  if (hasSSR && !window._commentsReloadNeeded) {
    const ssrCards = Array.from(bottomList.querySelectorAll(".cs-ssr-card"));
    const count = ssrCards.length;
    if (bottomCount) bottomCount.textContent = count > 0 ? String(count) : "";
    // Wire copy buttons on SSR code blocks
    bottomList.querySelectorAll(".cs-copy-btn").forEach(btn => {
      btn.addEventListener("click", () => {
        const pre = btn.previousElementSibling;
        if (pre) { navigator.clipboard.writeText(pre.textContent || ""); btn.textContent = "Copied!"; setTimeout(() => { btn.textContent = "Copy"; }, 2000); }
      });
    });
    // Wire helpful buttons on SSR cards
    bottomList.querySelectorAll(".cs-helpful-btn").forEach(btn => {
      const id = btn.dataset.id;
      const voted = getVoted();
      if (voted.has(Number(id))) btn.classList.add("voted");
      btn.addEventListener("click", async () => {
        if (voted.has(Number(id))) return;
        try {
          await fetch(`${API}/${id}/helpful`, { method: "POST" });
          const span = btn.querySelector(".helpful-count");
          if (span) span.textContent = String(Number(span.textContent || "0") + 1);
          voted.add(Number(id)); saveVoted(voted); btn.classList.add("voted");
        } catch {}
      });
    });
    // Team note slot buttons wired in DOMContentLoaded — always runs regardless of SSR cards
    // Populate sidebar from SSR cards (user notes only — team note is inline)
    if (sidebarList) {
      sidebarList.innerHTML = "";
      if (count === 0) {
        sidebarList.innerHTML = ``;
      } else {
        const pills = ssrCards.slice(0, 4).map(card => {
          const nameEl = card.querySelector(".cs-commenter-name");
          const verEl  = card.querySelector(".cs-version-badge");
          const name   = nameEl ? nameEl.textContent.replace(/^@/, "") : "anon";
          const ver    = verEl  ? verEl.textContent : "";
          return `@${escHtml(name)}${ver ? `${escHtml(ver)}` : ""}`;
        }).join("");
        const more = `See ${count} note${count>1?"s":""} ↓`;
        sidebarList.innerHTML = `${more}`;
      }
    }
    if (sidebarCount) sidebarCount.textContent = count > 0 ? String(count) : "";
    return;
  }

  if (bottomList) bottomList.innerHTML = renderSkeleton();

  try {
    const res  = await fetch(`${API}?url=${encodeURIComponent(PAGE_URL)}`);
    const data = await res.json();
    const raw = data.comments || [];
    // User notes only — team note is rendered inline in #team-note-slot by Worker
    const comments = raw.filter(c => c.name !== "theneuralbase_team");
    const voted = getVoted();

    // Update sidebar — user notes only
    if (sidebarList) {
      sidebarList.innerHTML = "";
      if (comments.length === 0) {
        sidebarList.innerHTML = ``;
      } else {
        const pills = comments.slice(0, 4).map(c => `
          @${escHtml(c.name||"anon")}
            ${c.version_tag ? `${escHtml(c.version_tag)}` : ""}
          `).join("");
        const more = comments.length > 0
          ? `
               See ${comments.length} note${comments.length>1?"s":""} ↓
             `
          : "";
        sidebarList.innerHTML = `${more}`;
      }
    }
    if (sidebarCount) sidebarCount.textContent = comments.length > 0 ? comments.length : "";

    // Update bottom full list
    if (bottomList) {
      bottomList.innerHTML = "";
      if (comments.length === 0) {
        bottomList.innerHTML = `No notes yetBe the first to share a version-specific fix or tip.`;
      } else {
        comments.forEach(c => bottomList.appendChild(renderCard(c, voted)));
      }
    }
    if (bottomCount) bottomCount.textContent = comments.length > 0 ? comments.length : "";

  } catch {
    if (bottomList) bottomList.innerHTML = `Could not load notesTry refreshing.`;
  }
}

function initForm(formEl) {
  if (!formEl || formEl.dataset.init) return;
  formEl.dataset.init = "1";
  const wrap     = formEl.closest(".cs-form-wrap");
  const addBtn   = wrap.querySelector(".cs-add-btn");
  const cancel   = formEl.querySelector(".cs-cancel-btn");
  const textarea = formEl.querySelector(".cs-comment");
  const charCount= formEl.querySelector(".cs-char-count");
  const submitBtn= formEl.querySelector(".cs-submit-btn");
  const errorEl  = formEl.querySelector(".cs-error");
  const hasCode  = formEl.querySelector(".cs-has-code");
  const codeWrap = formEl.querySelector(".cs-code-wrap");
  const codeArea = formEl.querySelector(".cs-code");

  addBtn.addEventListener("click", () => {
    addBtn.style.display = "none";
    formEl.style.display = "flex";
    formOpenedAt = Date.now();
    textarea.focus();
  });
  cancel.addEventListener("click", () => {
    addBtn.style.display = "block"; formEl.style.display = "none";
    formEl.reset(); if(codeWrap) codeWrap.style.display="none";
    errorEl.style.display="none"; charCount.textContent="0 / 1000";
  });
  textarea.addEventListener("input", () => { charCount.textContent=`${textarea.value.length} / 1000`; });
  if (hasCode && codeWrap) {
    hasCode.addEventListener("change", () => {
      codeWrap.style.display = hasCode.checked ? "flex" : "none";
      if (hasCode.checked && codeArea) codeArea.focus();
    });
  }
  formEl.addEventListener("submit", async (e) => {
    e.preventDefault();
    errorEl.style.display = "none";
    if (formEl.querySelector(".cs-hp").value) return;
    if (Date.now() - formOpenedAt  {
  loadComments();
  document.querySelectorAll(".cs-form").forEach(initForm);
  // Wire team note slot buttons unconditionally — runs on ALL pages regardless of user comments
  const teamSlot = document.getElementById("team-note-slot");
  if (teamSlot) {
    teamSlot.querySelectorAll(".cs-copy-btn").forEach(btn => {
      btn.addEventListener("click", () => {
        const wrap = btn.closest(".cs-comment-code-wrap");
        const pre = wrap ? wrap.querySelector("pre") : btn.previousElementSibling;
        if (pre) {
          navigator.clipboard.writeText(pre.textContent || "")
            .then(() => { btn.textContent = "Copied!"; btn.classList.add("copied"); setTimeout(() => { btn.textContent = "Copy"; btn.classList.remove("copied"); }, 2000); })
            .catch(() => {});
        }
      });
    });
    teamSlot.querySelectorAll(".cs-helpful-btn").forEach(btn => {
      const id = btn.dataset.id;
      const voted = getVoted();
      if (voted.has(Number(id))) btn.classList.add("voted");
      btn.addEventListener("click", async () => {
        if (voted.has(Number(id))) return;
        try {
          await fetch(`${API}/${id}/helpful`, { method: "POST" });
          const span = btn.querySelector(".helpful-count");
          if (span) span.textContent = String(Number(span.textContent || "0") + 1);
          voted.add(Number(id)); saveVoted(voted); btn.classList.add("voted");
        } catch {}
      });
    });
  }
});
})();
```
