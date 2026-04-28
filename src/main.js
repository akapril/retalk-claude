const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

let sessions = [];
let selectedIndex = 0;
let currentQuery = "";
let viewMode = localStorage.getItem("retalk_viewMode") || "project";
let sortMode = localStorage.getItem("retalk_sortMode") || "time";
let providerFilter = localStorage.getItem("retalk_providerFilter") || "all";

// 新功能状态
let favorites = [];       // Feature 3: 收藏的 session_id 列表
let allTags = {};          // Feature 6: session_id -> [tag1, tag2]
let gitInfoCache = {};     // Feature 2: project_path -> { branch, dirty_count }
let statsOpen = false;     // Feature 5: 统计面板状态
let contextSession = null; // Feature 4: 右键菜单关联的会话

const searchInput = document.getElementById("search-input");
const sessionList = document.getElementById("session-list");
const settingsBtn = document.getElementById("settings-btn");
const settingsPanel = document.getElementById("settings-panel");
const previewPanel = document.getElementById("preview-panel");
const previewMessages = document.getElementById("preview-messages");
const contextMenu = document.getElementById("context-menu");
const statsBtn = document.getElementById("stats-btn");
const statsPanel = document.getElementById("stats-panel");
const ddProvider = document.getElementById("dd-provider");
const ddView = document.getElementById("dd-view");
const ddSort = document.getElementById("dd-sort");
const appWindow = getCurrentWindow();

let settingsOpen = false;

// === 自定义图标下拉菜单 ===
function setupDropdown(container, currentValue, onChange) {
  const btn = container.querySelector(".icon-btn");
  const menu = container.querySelector(".icon-dropdown-menu");

  // 恢复 active 状态
  menu.querySelectorAll(".dd-item").forEach((item) => {
    item.classList.toggle("active", item.dataset.value === currentValue);
  });

  // 点击按钮切换菜单
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    // 关闭其他下拉
    document.querySelectorAll(".icon-dropdown.open").forEach((d) => {
      if (d !== container) d.classList.remove("open");
    });
    container.classList.toggle("open");
  });

  // 选择选项
  menu.querySelectorAll(".dd-item").forEach((item) => {
    item.addEventListener("click", (e) => {
      e.stopPropagation();
      const val = item.dataset.value;
      menu.querySelectorAll(".dd-item").forEach((i) => i.classList.remove("active"));
      item.classList.add("active");
      container.classList.remove("open");
      onChange(val);
    });
  });
}

// 点击外部关闭所有下拉
document.addEventListener("click", () => {
  document.querySelectorAll(".icon-dropdown.open").forEach((d) => d.classList.remove("open"));
});

setupDropdown(ddProvider, providerFilter, (val) => {
  providerFilter = val;
  localStorage.setItem("retalk_providerFilter", providerFilter);
  selectedIndex = 0;
  render();
});

setupDropdown(ddView, viewMode, (val) => {
  viewMode = val;
  localStorage.setItem("retalk_viewMode", viewMode);
  render();
});

setupDropdown(ddSort, sortMode, (val) => {
  sortMode = val;
  localStorage.setItem("retalk_sortMode", sortMode);
  render();
});

async function init() {
  // 加载收藏和标签
  try {
    favorites = await invoke("get_favorites");
  } catch (_) { /* 忽略 */ }
  try {
    allTags = await invoke("get_all_tags");
  } catch (_) { /* 忽略 */ }

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
  previewPanel.style.display = "none";
  settingsPanel.style.display = "";
  statsPanel.style.display = "none";
  statsOpen = false;

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
    await loadSessions();
  } catch (e) {
    console.error("保存配置失败:", e);
  }
});

// === Feature 5: 统计面板 ===
statsBtn.addEventListener("click", () => {
  if (statsOpen) {
    closeStats();
  } else {
    openStats();
  }
});

function openStats() {
  statsOpen = true;
  settingsOpen = false;
  sessionList.style.display = "none";
  previewPanel.style.display = "none";
  settingsPanel.style.display = "none";
  statsPanel.style.display = "";
  renderStats();
}

function closeStats() {
  statsOpen = false;
  statsPanel.style.display = "none";
  sessionList.style.display = "";
  searchInput.focus();
}

function renderStats() {
  // 按 provider 统计
  const byProvider = {};
  const byProject = {};
  const byMonth = {};

  sessions.forEach((s) => {
    // provider 统计
    byProvider[s.provider] = (byProvider[s.provider] || 0) + 1;
    // 项目统计
    const pName = s.project_name || "未知项目";
    byProject[pName] = (byProject[pName] || 0) + 1;
    // 月份统计（updated_at 格式: "MM-DD HH:MM"）
    const month = s.updated_at ? s.updated_at.substring(0, 2) : "??";
    byMonth[month] = (byMonth[month] || 0) + 1;
  });

  // 前 5 活跃项目
  const topProjects = Object.entries(byProject)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);

  const maxProjectCount = topProjects.length > 0 ? topProjects[0][1] : 1;

  // 月份排序
  const months = Object.entries(byMonth).sort((a, b) => a[0].localeCompare(b[0]));
  const maxMonthCount = months.length > 0 ? Math.max(...months.map((m) => m[1])) : 1;

  let html = `
    <div class="stats-section">
      <div class="stats-section-title">总览</div>
      <div class="stats-row"><span class="label">总会话数</span><span class="value">${sessions.length}</span></div>
      ${Object.entries(byProvider)
        .map(([p, c]) => `<div class="stats-row"><span class="label">${escapeHtml(p)}</span><span class="value">${c}</span></div>`)
        .join("")}
    </div>
    <div class="stats-section">
      <div class="stats-section-title">最活跃项目 (Top 5)</div>
      ${topProjects
        .map(
          ([name, count]) => `
        <div class="stats-bar-row">
          <div class="stats-bar-label">${escapeHtml(name)}</div>
          <div class="stats-bar-container">
            <div class="stats-bar-fill" style="width:${(count / maxProjectCount) * 100}%"></div>
            <span class="stats-bar-count">${count}</span>
          </div>
        </div>`
        )
        .join("")}
    </div>
    <div class="stats-section">
      <div class="stats-section-title">月度活跃</div>
      ${months
        .map(
          ([month, count]) => `
        <div class="stats-bar-row">
          <div class="stats-bar-label">${month} 月</div>
          <div class="stats-bar-container">
            <div class="stats-bar-fill" style="width:${(count / maxMonthCount) * 100}%"></div>
            <span class="stats-bar-count">${count}</span>
          </div>
        </div>`
        )
        .join("")}
    </div>
  `;

  statsPanel.innerHTML = html;
}

// === 会话加载 ===
async function loadSessions() {
  try {
    if (currentQuery.trim()) {
      sessions = await invoke("search", { query: currentQuery });
    } else {
      sessions = await invoke("list_sessions");
    }
    selectedIndex = 0;
    render();
    fetchGitInfoForVisible();
  } catch (e) {
    console.error("加载会话失败:", e);
  }
}

/// 按当前排序模式排序会话列表
function sortSessions(list) {
  if (sortMode === "name") {
    return [...list].sort((a, b) => a.project_name.localeCompare(b.project_name));
  }
  return [...list].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}

/// 按当前 provider 过滤
function filteredSessions() {
  let list = sessions;
  if (providerFilter !== "all") {
    list = list.filter((s) => s.provider === providerFilter);
  }
  return list;
}

/// Feature 3: 将收藏的会话排到前面
function applyFavoriteSort(list) {
  const favSet = new Set(favorites);
  const favItems = [];
  const normalItems = [];
  list.forEach((s) => {
    if (favSet.has(s.session_id)) {
      favItems.push(s);
    } else {
      normalItems.push(s);
    }
  });
  return [...favItems, ...normalItems];
}

function render() {
  sessionList.innerHTML = "";
  const list = applyFavoriteSort(filteredSessions());
  if (list.length === 0) {
    sessionList.innerHTML = '<div class="empty-state">没有找到会话</div>';
    previewPanel.style.display = "none";
    return;
  }
  if (viewMode === "project") {
    renderGrouped(list);
  } else {
    renderTimeline(list);
  }
  // 更新预览
  updatePreview();
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

  let sortedEntries = Object.entries(groups);
  if (sortMode === "name") {
    sortedEntries.sort(([a], [b]) => a.localeCompare(b));
  } else {
    sortedEntries.sort(([, a], [, b]) => {
      const latestA = a.reduce((max, s) => (s.updated_at > max ? s.updated_at : max), "");
      const latestB = b.reduce((max, s) => (s.updated_at > max ? s.updated_at : max), "");
      return latestB.localeCompare(latestA);
    });
  }

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
  item.dataset.sessionId = session.session_id;

  const isFav = favorites.includes(session.session_id);
  const promptText = session.last_prompt || session.first_prompt || "";
  const displayPrompt = highlightMatch(truncate(promptText, 80), currentQuery);
  const providerBadge = `<span class="provider-badge provider-${session.provider}">${session.provider}</span>`;

  // Feature 3: 收藏按钮
  const favBtnHtml = `<button class="fav-btn ${isFav ? "favorited" : ""}" data-sid="${session.session_id}" title="收藏">${isFav ? "★" : "☆"}</button>`;

  // Feature 2: Git 信息
  let gitHtml = "";
  const gi = gitInfoCache[session.project_path];
  if (gi) {
    gitHtml = `<span class="git-info"><span class="git-branch">${escapeHtml(gi.branch)}</span>`;
    if (gi.dirty_count > 0) {
      gitHtml += `<span class="git-dirty">${gi.dirty_count}</span>`;
    }
    gitHtml += `</span>`;
  }

  // Feature 6: 标签
  const sessionTags = allTags[session.session_id] || [];
  let tagsHtml = "";
  if (sessionTags.length > 0 || index === selectedIndex) {
    tagsHtml = `<div class="tags-row">`;
    sessionTags.forEach((t) => {
      tagsHtml += `<span class="tag-pill">${escapeHtml(t)}</span>`;
    });
    tagsHtml += `<button class="tag-add-btn" data-sid="${session.session_id}">+</button>`;
    tagsHtml += `</div>`;
  }

  // Token/成本估算显示
  let tokenHtml = "";
  if (session.total_tokens > 0) {
    const tokenStr = formatTokens(session.total_tokens);
    const costStr = estimateCost(session.provider, session.total_tokens);
    tokenHtml = `<span class="token-info">${tokenStr}${costStr ? " ~" + costStr : ""}</span>`;
  }

  item.innerHTML = `
    <div class="header">
      <span class="project-info">${favBtnHtml}${providerBadge}<span class="project-name">${escapeHtml(session.project_name)}</span>${gitHtml}</span>
      <span class="meta-right">${tokenHtml}<span class="time">${escapeHtml(session.updated_at)}</span></span>
    </div>
    <div class="prompt">${displayPrompt}</div>
    ${tagsHtml}
  `;

  // 点击恢复会话
  item.addEventListener("click", (e) => {
    // 不在星标/标签按钮上触发
    if (e.target.closest(".fav-btn") || e.target.closest(".tag-add-btn") || e.target.closest(".tag-pill")) return;
    selectedIndex = index;
    render();
    resumeSession(session);
  });

  // Feature 4: 右键菜单
  item.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    showContextMenu(e.clientX, e.clientY, session);
  });

  // Feature 3: 收藏按钮点击
  const favBtn = item.querySelector(".fav-btn");
  if (favBtn) {
    favBtn.addEventListener("click", async (e) => {
      e.stopPropagation();
      await toggleFavorite(session.session_id);
    });
  }

  // Feature 6: 标签编辑按钮
  const tagAddBtn = item.querySelector(".tag-add-btn");
  if (tagAddBtn) {
    tagAddBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      startTagEdit(session.session_id, item);
    });
  }

  // Feature 6: 标签点击过滤搜索
  item.querySelectorAll(".tag-pill").forEach((pill) => {
    pill.addEventListener("click", (e) => {
      e.stopPropagation();
      searchInput.value = pill.textContent;
      currentQuery = pill.textContent;
      loadSessions();
    });
  });

  return item;
}

// ======================== Feature 1: 预览面板 ========================

let previewTimer = null;
let lastPreviewId = "";

function updatePreview() {
  clearTimeout(previewTimer);
  const list = applyFavoriteSort(filteredSessions());
  const current = list[selectedIndex];
  if (!current) {
    previewPanel.style.display = "none";
    return;
  }
  if (current.session_id === lastPreviewId) return;

  previewTimer = setTimeout(async () => {
    try {
      const msgs = await invoke("get_session_preview", {
        sessionId: current.session_id,
      });
      lastPreviewId = current.session_id;
      if (msgs.length === 0) {
        previewPanel.style.display = "none";
        return;
      }
      previewMessages.innerHTML = msgs
        .map((m) => `<div class="preview-msg">${escapeHtml(truncate(m, 120))}</div>`)
        .join("");
      if (!settingsOpen && !statsOpen) {
        previewPanel.style.display = "";
      }
    } catch (_) {
      previewPanel.style.display = "none";
    }
  }, 200);
}

// ======================== Feature 2: Git 信息 ========================

let gitFetching = false;
async function fetchGitInfoForVisible() {
  if (gitFetching) return;
  gitFetching = true;

  // 收集未缓存的 project_path
  const paths = [...new Set(sessions.map((s) => s.project_path))]
    .filter((p) => !gitInfoCache[p] && !p.startsWith("gemini:")); // gemini 无真实路径

  // 并行请求（最多 5 个并发）
  const batchSize = 5;
  let updated = false;
  for (let i = 0; i < paths.length; i += batchSize) {
    const batch = paths.slice(i, i + batchSize);
    const results = await Promise.allSettled(
      batch.map((p) => invoke("get_project_git_info", { projectPath: p }).then((info) => ({ p, info })))
    );
    for (const r of results) {
      if (r.status === "fulfilled" && r.value.info) {
        gitInfoCache[r.value.p] = r.value.info;
        updated = true;
      }
    }
  }

  gitFetching = false;
  if (updated) render();
}

// ======================== Feature 3: 收藏 ========================

async function toggleFavorite(sessionId) {
  try {
    await invoke("toggle_favorite", { sessionId });
    favorites = await invoke("get_favorites");
    render();
  } catch (e) {
    console.error("收藏操作失败:", e);
  }
}

// ======================== Feature 4: 右键菜单 ========================

function showContextMenu(x, y, session) {
  contextSession = session;
  contextMenu.style.display = "";
  contextMenu.style.left = `${x}px`;
  contextMenu.style.top = `${y}px`;

  // 防止溢出窗口
  const rect = contextMenu.getBoundingClientRect();
  if (rect.right > window.innerWidth) {
    contextMenu.style.left = `${window.innerWidth - rect.width - 4}px`;
  }
  if (rect.bottom > window.innerHeight) {
    contextMenu.style.top = `${window.innerHeight - rect.height - 4}px`;
  }
}

function hideContextMenu() {
  contextMenu.style.display = "none";
  contextSession = null;
}

// 右键菜单事件绑定
contextMenu.querySelectorAll(".ctx-item").forEach((item) => {
  item.addEventListener("click", async () => {
    if (!contextSession) return;
    const action = item.dataset.action;
    const s = contextSession;
    hideContextMenu();

    switch (action) {
      case "resume":
        await resumeSession(s);
        break;
      case "vscode":
        try {
          await invoke("open_in_vscode", { projectPath: s.project_path });
        } catch (e) {
          console.error("打开 VS Code 失败:", e);
        }
        break;
      case "explorer":
        try {
          await invoke("open_in_explorer", { projectPath: s.project_path });
        } catch (e) {
          console.error("打开文件管理器失败:", e);
        }
        break;
      case "copy-path":
        if (navigator.clipboard) {
          await navigator.clipboard.writeText(s.project_path);
          showToast("已复制项目路径");
        }
        break;
      case "copy-cmd":
        await copyCommand(s);
        showToast("已复制恢复命令");
        break;
      case "export-md":
        try {
          const md = await invoke("export_session_markdown", { sessionId: s.session_id });
          if (navigator.clipboard) {
            await navigator.clipboard.writeText(md);
            showToast("已复制 Markdown 到剪贴板");
          }
        } catch (e) {
          showToast("导出失败: " + e);
        }
        break;
      case "export-file":
        try {
          const desktop = await invoke("get_desktop_path");
          const fileName = `${s.project_name}_${s.session_id.slice(0, 8)}.md`;
          const filePath = `${desktop}\\${fileName}`;
          await invoke("export_session_to_file", { sessionId: s.session_id, filePath });
          showToast("已保存到桌面: " + fileName);
          // 用资源管理器打开并选中文件
          try {
            await invoke("open_in_explorer_select", { filePath });
          } catch (_) {}
        } catch (e) {
          showToast("导出失败: " + e);
        }
        break;
    }
  });
});

// 点击其他地方或按 Esc 关闭菜单
document.addEventListener("click", (e) => {
  if (!contextMenu.contains(e.target)) {
    hideContextMenu();
  }
});

// ======================== Feature 6: 标签编辑 ========================

function startTagEdit(sessionId, itemEl) {
  const tagsRow = itemEl.querySelector(".tags-row");
  if (!tagsRow) return;

  const currentTags = allTags[sessionId] || [];
  const input = document.createElement("input");
  input.className = "tag-input";
  input.value = currentTags.join(", ");
  input.placeholder = "逗号分隔标签...";

  // 隐藏标签按钮，显示输入框
  const addBtn = tagsRow.querySelector(".tag-add-btn");
  if (addBtn) addBtn.style.display = "none";

  tagsRow.appendChild(input);
  input.focus();

  const commit = async () => {
    const raw = input.value.trim();
    const tags = raw
      ? raw
          .split(/[,，]/)
          .map((t) => t.trim())
          .filter((t) => t)
      : [];
    try {
      await invoke("set_tags", { sessionId, tags });
      allTags = await invoke("get_all_tags");
    } catch (e) {
      console.error("保存标签失败:", e);
    }
    render();
  };

  input.addEventListener("blur", commit);
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      input.blur();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      input.value = currentTags.join(", ");
      input.blur();
    }
    e.stopPropagation(); // 防止触发全局快捷键
  });
}

// ======================== 搜索与导航 ========================

let searchTimer = null;
searchInput.addEventListener("input", () => {
  currentQuery = searchInput.value;
  clearTimeout(searchTimer);
  searchTimer = setTimeout(loadSessions, 150);
});

// 下拉选择已通过 setupDropdown 处理

document.addEventListener("keydown", async (e) => {
  // 在输入框中不拦截方向键（标签输入框和搜索框均跳过）
  if (e.target.classList.contains("tag-input")) return;
  const inSearchBox = e.target === searchInput;

  if (e.key === "ArrowDown" && !inSearchBox) {
    e.preventDefault();
    if (selectedIndex < sessions.length - 1) {
      selectedIndex++;
      lastPreviewId = ""; // 强制刷新预览
      render();
      scrollToSelected();
    }
  } else if (e.key === "ArrowUp" && !inSearchBox) {
    e.preventDefault();
    if (selectedIndex > 0) {
      selectedIndex--;
      lastPreviewId = "";
      render();
      scrollToSelected();
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    const list = applyFavoriteSort(filteredSessions());
    if (list[selectedIndex]) {
      await resumeSession(list[selectedIndex]);
    }
  } else if (e.key === "c" && e.ctrlKey) {
    e.preventDefault();
    const list = applyFavoriteSort(filteredSessions());
    if (list[selectedIndex]) {
      await copyCommand(list[selectedIndex]);
    }
  } else if (e.key === "Escape") {
    if (contextMenu.style.display !== "none") {
      hideContextMenu();
    } else if (settingsOpen) {
      closeSettings();
    } else if (statsOpen) {
      closeStats();
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

// ======================== 工具函数 ========================

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
  const regex = new RegExp(`(${queryEscaped.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi");
  return escaped.replace(regex, "<mark>$1</mark>");
}

// ======================== Token/成本估算 ========================

/// 格式化 token 数为可读字符串
function formatTokens(tokens) {
  if (tokens >= 1000000) {
    return (tokens / 1000000).toFixed(1) + "M tokens";
  } else if (tokens >= 1000) {
    return (tokens / 1000).toFixed(1) + "k tokens";
  }
  return tokens + " tokens";
}

/// 根据 provider 估算成本（粗略估算，假设 input:output = 1:1）
function estimateCost(provider, tokens) {
  if (tokens === 0) return "";
  // 粗略按总 token 的一半为 input、一半为 output 估算
  const half = tokens / 2;
  let cost = 0;
  switch (provider) {
    case "claude":
      // ~$3/M input + $15/M output
      cost = (half * 3 + half * 15) / 1000000;
      break;
    case "codex":
      // ~$2/M input + $8/M output
      cost = (half * 2 + half * 8) / 1000000;
      break;
    default:
      return "";
  }
  if (cost < 0.01) return "$<0.01";
  return "$" + cost.toFixed(2);
}

/// 操作反馈提示
function showToast(message) {
  let toast = document.getElementById("toast");
  if (!toast) {
    toast = document.createElement("div");
    toast.id = "toast";
    toast.className = "toast";
    document.getElementById("app").appendChild(toast);
  }
  toast.textContent = message;
  toast.classList.add("show");
  setTimeout(() => toast.classList.remove("show"), 2000);
}

// 窗口可见时刷新
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    searchInput.focus();
    loadSessions();
  }
});

init();
