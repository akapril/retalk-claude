const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

let sessions = [];
let selectedIndex = 0;
let currentQuery = "";
let viewMode = "timeline";

const searchInput = document.getElementById("search-input");
const sessionList = document.getElementById("session-list");
const viewModeSelect = document.getElementById("view-mode");
const appWindow = getCurrentWindow();

async function init() {
  await loadSessions();
  searchInput.focus();
}

async function loadSessions() {
  try {
    if (currentQuery.trim()) {
      sessions = await invoke("search", { query: currentQuery });
    } else {
      sessions = await invoke("list_sessions");
    }
    selectedIndex = 0;
    render();
  } catch (e) {
    console.error("加载会话失败:", e);
  }
}

function render() {
  sessionList.innerHTML = "";
  if (sessions.length === 0) {
    sessionList.innerHTML = '<div class="empty-state">没有找到会话</div>';
    return;
  }
  if (viewMode === "project") {
    renderGrouped();
  } else {
    renderTimeline();
  }
}

function renderTimeline() {
  sessions.forEach((s, i) => {
    sessionList.appendChild(createSessionItem(s, i));
  });
}

function renderGrouped() {
  const groups = {};
  sessions.forEach((s) => {
    const key = s.project_name || "未知项目";
    if (!groups[key]) groups[key] = [];
    groups[key].push(s);
  });

  let globalIdx = 0;
  Object.entries(groups).forEach(([name, items]) => {
    const header = document.createElement("div");
    header.className = "group-header";
    header.textContent = `${name} (${items.length})`;
    sessionList.appendChild(header);
    items.forEach((s) => {
      sessionList.appendChild(createSessionItem(s, globalIdx));
      globalIdx++;
    });
  });
}

function createSessionItem(session, index) {
  const item = document.createElement("div");
  item.className = "session-item" + (index === selectedIndex ? " selected" : "");
  item.dataset.index = index;

  const promptText = session.last_prompt || session.first_prompt || "";
  const displayPrompt = highlightMatch(truncate(promptText, 80), currentQuery);

  const providerBadge = `<span class="provider-badge provider-${session.provider}">${session.provider}</span>`;

  item.innerHTML = `
    <div class="header">
      <span class="project-info">${providerBadge}<span class="project-name">${escapeHtml(session.project_name)}</span></span>
      <span class="time">${escapeHtml(session.updated_at)}</span>
    </div>
    <div class="prompt">${displayPrompt}</div>
  `;

  item.addEventListener("click", () => {
    selectedIndex = index;
    render();
    resumeSession(session);
  });

  return item;
}

let searchTimer = null;
searchInput.addEventListener("input", () => {
  currentQuery = searchInput.value;
  clearTimeout(searchTimer);
  searchTimer = setTimeout(loadSessions, 150);
});

viewModeSelect.addEventListener("change", () => {
  viewMode = viewModeSelect.value;
  render();
});

document.addEventListener("keydown", async (e) => {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (selectedIndex < sessions.length - 1) {
      selectedIndex++;
      render();
      scrollToSelected();
    }
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (selectedIndex > 0) {
      selectedIndex--;
      render();
      scrollToSelected();
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (sessions[selectedIndex]) {
      await resumeSession(sessions[selectedIndex]);
    }
  } else if (e.key === "c" && e.ctrlKey) {
    e.preventDefault();
    if (sessions[selectedIndex]) {
      await copyCommand(sessions[selectedIndex]);
    }
  } else if (e.key === "Escape") {
    await appWindow.hide();
  }
});

function scrollToSelected() {
  const selected = sessionList.querySelector(".selected");
  if (selected) {
    selected.scrollIntoView({ block: "nearest" });
  }
}

async function resumeSession(session) {
  try {
    await invoke("resume_session", {
      sessionId: session.session_id,
      projectPath: session.project_path,
      provider: session.provider,
    });
    await appWindow.hide();
  } catch (e) {
    console.error("恢复会话失败:", e);
  }
}

async function copyCommand(session) {
  try {
    const cmd = await invoke("copy_command", {
      sessionId: session.session_id,
      projectPath: session.project_path,
      provider: session.provider,
    });
    if (navigator.clipboard) {
      await navigator.clipboard.writeText(cmd);
    }
  } catch (e) {
    console.error("复制失败:", e);
  }
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str || "";
  return div.innerHTML;
}

function truncate(str, max) {
  if (!str) return "";
  return str.length > max ? str.slice(0, max) + "..." : str;
}

function highlightMatch(text, query) {
  if (!query || !query.trim()) return escapeHtml(text);
  const escaped = escapeHtml(text);
  const queryEscaped = escapeHtml(query.trim());
  const regex = new RegExp(`(${queryEscaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, "gi");
  return escaped.replace(regex, "<mark>$1</mark>");
}

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    searchInput.focus();
    loadSessions();
  }
});

init();
