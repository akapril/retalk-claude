const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

let sessions = [];
let selectedIndex = 0;
let currentQuery = "";
let viewMode = "project";
let sortMode = "time"; // "time" | "name"
let providerFilter = "all"; // "all" | "claude" | "codex" | "gemini" | "continue"

const searchInput = document.getElementById("search-input");
const sessionList = document.getElementById("session-list");
const viewModeSelect = document.getElementById("view-mode");
const sortModeSelect = document.getElementById("sort-mode");
const providerFilterSelect = document.getElementById("provider-filter");
const settingsBtn = document.getElementById("settings-btn");
const settingsPanel = document.getElementById("settings-panel");
const appWindow = getCurrentWindow();

let settingsOpen = false;

async function init() {
  await loadSessions();
  searchInput.focus();
}

// === 设置面板 ===
settingsBtn.addEventListener("click", async () => {
  if (settingsOpen) {
    closeSettings();
  } else {
    await openSettings();
  }
});

async function openSettings() {
  settingsOpen = true;
  sessionList.style.display = "none";
  settingsPanel.style.display = "";

  // 加载当前配置
  try {
    const config = await invoke("get_config");
    document.getElementById("cfg-hotkey").value = config.general.hotkey;
    document.getElementById("cfg-terminal").value = config.terminal.preferred;
    document.getElementById("cfg-watcher").checked = config.update.watcher_enabled;
    document.getElementById("cfg-poll").checked = config.update.poll_enabled;
    document.getElementById("cfg-poll-interval").value = config.update.poll_interval_secs;
    document.getElementById("cfg-ondemand").checked = config.update.on_demand_enabled;
    document.getElementById("cfg-max-results").value = config.ui.max_results;
  } catch (e) {
    console.error("加载配置失败:", e);
  }
}

function closeSettings() {
  settingsOpen = false;
  settingsPanel.style.display = "none";
  sessionList.style.display = "";
  searchInput.focus();
}

document.getElementById("settings-cancel").addEventListener("click", closeSettings);

document.getElementById("settings-save").addEventListener("click", async () => {
  const newConfig = {
    general: {
      hotkey: document.getElementById("cfg-hotkey").value,
    },
    terminal: {
      preferred: document.getElementById("cfg-terminal").value,
    },
    update: {
      watcher_enabled: document.getElementById("cfg-watcher").checked,
      poll_enabled: document.getElementById("cfg-poll").checked,
      poll_interval_secs: parseInt(document.getElementById("cfg-poll-interval").value) || 30,
      on_demand_enabled: document.getElementById("cfg-ondemand").checked,
    },
    ui: {
      theme: "dark",
      max_results: parseInt(document.getElementById("cfg-max-results").value) || 50,
    },
  };

  try {
    await invoke("save_config", { newConfig });
    closeSettings();
    await loadSessions(); // 刷新列表（max_results 可能变了）
  } catch (e) {
    console.error("保存配置失败:", e);
  }
});

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

/// 按当前排序模式排序会话列表
function sortSessions(list) {
  if (sortMode === "name") {
    return [...list].sort((a, b) => a.project_name.localeCompare(b.project_name));
  }
  // 默认按时间降序（后端已排好，但切换排序后需重排）
  return [...list].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}

/// 按当前 provider 过滤
function filteredSessions() {
  if (providerFilter === "all") return sessions;
  return sessions.filter(s => s.provider === providerFilter);
}

function render() {
  sessionList.innerHTML = "";
  const list = filteredSessions();
  if (list.length === 0) {
    sessionList.innerHTML = '<div class="empty-state">没有找到会话</div>';
    return;
  }
  if (viewMode === "project") {
    renderGrouped(list);
  } else {
    renderTimeline(list);
  }
}

function renderTimeline(list) {
  const sorted = sortSessions(list);
  sorted.forEach((s, i) => {
    sessionList.appendChild(createSessionItem(s, i));
  });
}

function renderGrouped(list) {
  const groups = {};
  list.forEach((s) => {
    const key = s.project_name || "未知项目";
    if (!groups[key]) groups[key] = [];
    groups[key].push(s);
  });

  // 对分组排序：按名称排序时按组名字母序，按时间排序时按组内最新会话时间降序
  let sortedEntries = Object.entries(groups);
  if (sortMode === "name") {
    sortedEntries.sort(([a], [b]) => a.localeCompare(b));
  } else {
    sortedEntries.sort(([, a], [, b]) => {
      const latestA = a.reduce((max, s) => s.updated_at > max ? s.updated_at : max, "");
      const latestB = b.reduce((max, s) => s.updated_at > max ? s.updated_at : max, "");
      return latestB.localeCompare(latestA);
    });
  }

  // 组内会话也按当前排序
  let globalIdx = 0;
  sortedEntries.forEach(([name, items]) => {
    const sortedItems = sortSessions(items);
    const header = document.createElement("div");
    header.className = "group-header";
    header.textContent = `${name} (${sortedItems.length})`;
    sessionList.appendChild(header);
    sortedItems.forEach((s) => {
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

sortModeSelect.addEventListener("change", () => {
  sortMode = sortModeSelect.value;
  render();
});

providerFilterSelect.addEventListener("change", () => {
  providerFilter = providerFilterSelect.value;
  selectedIndex = 0;
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
    if (settingsOpen) {
      closeSettings();
    } else {
      await appWindow.hide();
    }
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
