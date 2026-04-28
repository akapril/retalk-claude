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
let compareOpen = false;   // Feature 5(会话对比): 对比视图状态
let multiSelectMode = false; // Feature 6(批量操作): 多选模式
let multiSelected = new Set(); // Feature 6: 已选会话 ID 集合
let providerStatus = [];   // Feature 1(空状态引导): provider 可用状态

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
const statusBar = document.getElementById("status-bar");
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
  loadSessions(); // 重新从后端查询，按 provider 过滤
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
  // 加载 provider 状态（空状态引导用）
  try {
    providerStatus = await invoke("get_provider_status");
  } catch (_) { /* 忽略 */ }

  // 等待后台扫描完成再加载数据
  await waitForReady();
  dataReady = true;
  await loadSessions();
  searchInput.focus();
}

/// 等待后台数据扫描完成
async function waitForReady() {
  let ready = false;
  while (!ready) {
    try {
      ready = await invoke("is_ready");
    } catch (_) {}
    if (!ready) {
      sessionList.innerHTML = '<div class="empty-state">正在扫描会话数据...</div>';
      await new Promise((r) => setTimeout(r, 300));
    }
  }
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
  compareOpen = false;
  updateStatusBar();

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

  // Feature 8: 加载开机自启状态
  try {
    document.getElementById("cfg-autostart").checked = await invoke("get_autostart");
  } catch (_) { /* 忽略 */ }
}

function closeSettings() {
  settingsOpen = false;
  settingsPanel.style.display = "none";
  sessionList.style.display = "";
  searchInput.focus();
  updateStatusBar();
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
    // Feature 8: 保存开机自启状态
    const autostart = document.getElementById("cfg-autostart").checked;
    await invoke("set_autostart", { enabled: autostart });
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
  compareOpen = false;
  sessionList.style.display = "none";
  previewPanel.style.display = "none";
  settingsPanel.style.display = "none";
  statsPanel.style.display = "";
  renderStats();
  updateStatusBar();
}

function closeStats() {
  statsOpen = false;
  statsPanel.style.display = "none";
  sessionList.style.display = "";
  searchInput.focus();
  updateStatusBar();
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

  // Feature 10: 项目健康度 — 频繁活动 vs 长期未活动
  const now = new Date();
  const oneWeekAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
  const twoWeeksAgo = new Date(now.getTime() - 14 * 24 * 60 * 60 * 1000);
  const thirtyDaysAgo = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);

  // 解析 updated_at（"MM-DD HH:MM" 格式）为当前年份的 Date
  function parseSessionDate(dateStr) {
    if (!dateStr || dateStr.length < 11) return null;
    const month = parseInt(dateStr.substring(0, 2), 10);
    const day = parseInt(dateStr.substring(3, 5), 10);
    const hour = parseInt(dateStr.substring(6, 8), 10);
    const min = parseInt(dateStr.substring(9, 11), 10);
    if (isNaN(month) || isNaN(day)) return null;
    return new Date(now.getFullYear(), month - 1, day, hour || 0, min || 0);
  }

  // 按项目分组统计本周 vs 上周
  const projectThisWeek = {};
  const projectLastWeek = {};
  const projectLatest = {}; // 项目最近会话日期
  sessions.forEach((s) => {
    const pName = s.project_name || "未知项目";
    const d = parseSessionDate(s.updated_at);
    if (!d) return;
    // 记录最新日期
    if (!projectLatest[pName] || d > projectLatest[pName]) {
      projectLatest[pName] = d;
    }
    if (d >= oneWeekAgo) {
      projectThisWeek[pName] = (projectThisWeek[pName] || 0) + 1;
    } else if (d >= twoWeeksAgo) {
      projectLastWeek[pName] = (projectLastWeek[pName] || 0) + 1;
    }
  });

  // 频繁活动项目：本周会话数 > 上周
  const hotProjects = Object.entries(projectThisWeek)
    .filter(([name, cnt]) => cnt > (projectLastWeek[name] || 0))
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);

  // 长期未活动：所有会话都超过 30 天
  const coldProjects = Object.entries(projectLatest)
    .filter(([, d]) => d < thirtyDaysAgo)
    .map(([name]) => name)
    .slice(0, 5);

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

  // Feature 10: 项目健康度
  if (hotProjects.length > 0 || coldProjects.length > 0) {
    html += `<div class="stats-section"><div class="stats-section-title">项目健康度</div>`;
    if (hotProjects.length > 0) {
      html += hotProjects.map(([name, cnt]) =>
        `<div class="stats-row"><span class="label">${escapeHtml(name)}<span class="stats-tag stats-tag-hot">本周 ${cnt} 次</span></span><span class="value">频繁活动</span></div>`
      ).join("");
    }
    if (coldProjects.length > 0) {
      html += coldProjects.map((name) =>
        `<div class="stats-row"><span class="label">${escapeHtml(name)}<span class="stats-tag stats-tag-cold">30+ 天未活动</span></span><span class="value">休眠</span></div>`
      ).join("");
    }
    html += `</div>`;
  }

  statsPanel.innerHTML = html;
}

// === 会话加载 ===
let dataReady = false;

async function loadSessions() {
  // 后台扫描未完成时跳过，避免锁竞争导致卡死
  if (!dataReady) return;
  try {
    if (currentQuery.trim()) {
      sessions = await invoke("search", { query: currentQuery, providerFilter });
    } else {
      sessions = await invoke("list_sessions", { providerFilter });
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
    // Feature 1: 空状态引导 — 区分不同原因
    const hasProviders = providerStatus.some((p) => p.available);
    if (currentQuery.trim()) {
      sessionList.innerHTML = '<div class="empty-state">没有找到匹配的会话</div>';
    } else if (!hasProviders && providerStatus.length > 0) {
      const names = providerStatus.map((p) => p.name).join(", ");
      sessionList.innerHTML = `<div class="empty-state-detail">
        <div>未检测到 AI 编码工具</div>
        <div class="providers-list">支持: ${escapeHtml(names)}</div>
      </div>`;
    } else {
      sessionList.innerHTML = '<div class="empty-state">已扫描完成，暂无会话记录</div>';
    }
    previewPanel.style.display = "none";
    updateStatusBar();
    return;
  }
  if (viewMode === "project") {
    renderGrouped(list);
  } else {
    renderTimeline(list);
  }
  // 更新预览和状态栏
  updatePreview();
  updateStatusBar();
}

function renderTimeline(list) {
  // 先按收藏分组，组内再排序，保证收藏始终在顶部
  const favSet = new Set(favorites);
  const favItems = sortSessions(list.filter((s) => favSet.has(s.session_id)));
  const normalItems = sortSessions(list.filter((s) => !favSet.has(s.session_id)));
  const sorted = [...favItems, ...normalItems];

  // Feature 3: 时间分组
  let lastGroup = "";
  sorted.forEach((s, i) => {
    const group = getTimeGroup(s.updated_at);
    if (group !== lastGroup) {
      const header = document.createElement("div");
      header.className = "group-header";
      header.textContent = group;
      sessionList.appendChild(header);
      lastGroup = group;
    }
    sessionList.appendChild(createSessionItem(s, i));
  });
}

/// Feature 3: 根据日期字符串返回时间分组标签
function getTimeGroup(dateStr) {
  if (!dateStr || dateStr.length < 11) return "更早";
  const now = new Date();
  const month = parseInt(dateStr.substring(0, 2), 10);
  const day = parseInt(dateStr.substring(3, 5), 10);
  if (isNaN(month) || isNaN(day)) return "更早";

  const sessionDate = new Date(now.getFullYear(), month - 1, day);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const diffDays = Math.floor((today - sessionDate) / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "今天";
  if (diffDays === 1) return "昨天";
  if (diffDays < 7) return "本周";
  if (diffDays < 30) return "本月";
  return "更早";
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

  const favSet = new Set(favorites);
  let globalIdx = 0;
  sortedEntries.forEach(([name, items]) => {
    // 组内：收藏在前，各自按排序模式排
    const favInGroup = sortSessions(items.filter((s) => favSet.has(s.session_id)));
    const normalInGroup = sortSessions(items.filter((s) => !favSet.has(s.session_id)));
    const sortedItems = [...favInGroup, ...normalInGroup];
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
  const isMultiSel = multiSelected.has(session.session_id);
  item.className = "session-item" + (index === selectedIndex ? " selected" : "") + (isMultiSel ? " multi-selected" : "");
  item.dataset.index = index;
  item.dataset.sessionId = session.session_id;

  const isFav = favorites.includes(session.session_id);
  const promptText = session.last_prompt || session.first_prompt || "";
  const displayPrompt = highlightMatch(truncate(promptText, 80), currentQuery);
  const providerBadge = `<span class="provider-badge provider-${session.provider}">${session.provider}</span>`;

  // Feature 6(批量): 多选复选框
  const checkboxHtml = multiSelectMode
    ? `<input type="checkbox" class="multi-select-checkbox" ${isMultiSel ? "checked" : ""} data-sid="${session.session_id}" />`
    : "";

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
      <span class="project-info">${checkboxHtml}${favBtnHtml}${providerBadge}<span class="project-name">${escapeHtml(session.project_name)}</span>${gitHtml}</span>
      <span class="meta-right">${tokenHtml}<span class="time">${escapeHtml(session.updated_at)}</span></span>
    </div>
    <div class="prompt">${displayPrompt}</div>
    ${tagsHtml}
  `;

  // 点击恢复会话 / Ctrl+Click 多选
  item.addEventListener("click", (e) => {
    // 不在星标/标签按钮/复选框上触发
    if (e.target.closest(".fav-btn") || e.target.closest(".tag-add-btn") || e.target.closest(".tag-pill") || e.target.closest(".multi-select-checkbox")) return;

    // Feature 6: Ctrl+Click 进入多选模式
    if (e.ctrlKey) {
      toggleMultiSelect(session.session_id);
      return;
    }

    if (multiSelectMode) {
      toggleMultiSelect(session.session_id);
      return;
    }

    selectedIndex = index;
    render();
    resumeSession(session);
  });

  // Feature 6: 复选框点击
  const checkbox = item.querySelector(".multi-select-checkbox");
  if (checkbox) {
    checkbox.addEventListener("click", (e) => {
      e.stopPropagation();
      toggleMultiSelect(session.session_id);
    });
  }

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
        .map((m) => `<div class="preview-msg">${highlightMatch(truncate(m, 120), currentQuery)}</div>`)
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
      case "compare":
        openCompareView(s);
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
    } else if (multiSelectMode) {
      exitMultiSelect();
    } else if (compareOpen) {
      closeCompareView();
    } else if (settingsOpen) {
      closeSettings();
    } else if (statsOpen) {
      closeStats();
    } else if (currentQuery.trim()) {
      // Feature 2(动态状态栏): 搜索模式按 Esc 清空搜索
      searchInput.value = "";
      currentQuery = "";
      loadSessions();
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

// ======================== Feature 2: 动态 Status Bar ========================

function updateStatusBar() {
  if (!statusBar) return;
  if (multiSelectMode) {
    statusBar.innerHTML = `<span>已选 ${multiSelected.size} 项</span><span>Esc 取消</span>`;
    return;
  }
  if (settingsOpen) {
    statusBar.innerHTML = `<span>Esc 返回</span>`;
    return;
  }
  if (statsOpen) {
    statusBar.innerHTML = `<span>Esc 返回</span>`;
    return;
  }
  if (compareOpen) {
    statusBar.innerHTML = `<span>Esc 返回</span>`;
    return;
  }
  if (currentQuery.trim()) {
    statusBar.innerHTML = `<span>Enter 恢复</span><span>Esc 清空搜索</span>`;
    return;
  }
  statusBar.innerHTML = `<span>↑↓ 导航</span><span>Enter 恢复</span><span>Ctrl+C 复制命令</span><span>Esc 关闭</span>`;
}

// ======================== Feature 5: 会话对比视图 ========================

function openCompareView(session) {
  // 找到同项目不同 provider 的所有会话
  const projectPath = session.project_path;
  const projectSessions = sessions.filter((s) => s.project_path === projectPath);

  // 按 provider 分组
  const byProvider = {};
  projectSessions.forEach((s) => {
    if (!byProvider[s.provider]) byProvider[s.provider] = [];
    byProvider[s.provider].push(s);
  });

  const providers = Object.keys(byProvider);
  if (providers.length < 2) {
    showToast("该项目仅有一个工具的会话，无需对比");
    return;
  }

  compareOpen = true;
  sessionList.style.display = "none";
  previewPanel.style.display = "none";
  settingsPanel.style.display = "none";
  statsPanel.style.display = "none";

  // 创建/复用对比容器
  let compareEl = document.getElementById("compare-view");
  if (!compareEl) {
    compareEl = document.createElement("div");
    compareEl.id = "compare-view";
    compareEl.className = "compare-view";
    sessionList.parentNode.insertBefore(compareEl, previewPanel);
  }
  compareEl.style.display = "";

  let html = `
    <div class="compare-header">
      <span class="project-title">${escapeHtml(session.project_name)}</span>
      <button class="compare-back-btn" id="compare-back">返回</button>
    </div>
    <div class="compare-grid">
  `;

  providers.forEach((prov) => {
    const pSessions = byProvider[prov];
    const latest = pSessions.reduce((a, b) => (a.updated_at > b.updated_at ? a : b));
    const msgs = latest.user_messages || [];
    const previewMsgs = msgs.slice(-3);

    html += `
      <div class="compare-column">
        <div class="compare-provider">
          <span class="provider-badge provider-${prov}">${escapeHtml(prov)}</span>
          (${pSessions.length} 次)
        </div>
        <div class="compare-meta">最近: ${escapeHtml(latest.updated_at)}</div>
        ${previewMsgs.map((m) => `<div class="compare-msg">${escapeHtml(truncate(m, 60))}</div>`).join("")}
      </div>
    `;
  });

  html += `</div>`;
  compareEl.innerHTML = html;

  document.getElementById("compare-back").addEventListener("click", closeCompareView);
  updateStatusBar();
}

function closeCompareView() {
  compareOpen = false;
  const compareEl = document.getElementById("compare-view");
  if (compareEl) compareEl.style.display = "none";
  sessionList.style.display = "";
  searchInput.focus();
  updateStatusBar();
}

// ======================== Feature 6: 批量操作 ========================

function toggleMultiSelect(sessionId) {
  if (!multiSelectMode) {
    multiSelectMode = true;
  }
  if (multiSelected.has(sessionId)) {
    multiSelected.delete(sessionId);
  } else {
    multiSelected.add(sessionId);
  }
  if (multiSelected.size === 0) {
    exitMultiSelect();
    return;
  }
  render();
  showMultiBar();
}

function exitMultiSelect() {
  multiSelectMode = false;
  multiSelected.clear();
  hideMultiBar();
  render();
}

function showMultiBar() {
  let bar = document.getElementById("multi-bar");
  if (!bar) {
    bar = document.createElement("div");
    bar.id = "multi-bar";
    bar.className = "multi-bar";
    // 插入到 status-bar 前面
    statusBar.parentNode.insertBefore(bar, statusBar);
  }
  bar.innerHTML = `
    <span>已选 ${multiSelected.size} 项</span>
    <button class="multi-bar-btn primary" id="multi-export">导出</button>
    <button class="multi-bar-btn" id="multi-cancel">取消</button>
  `;
  bar.style.display = "";

  document.getElementById("multi-export").addEventListener("click", async () => {
    try {
      const ids = Array.from(multiSelected);
      const md = await invoke("batch_export_markdown", { sessionIds: ids });
      if (navigator.clipboard) {
        await navigator.clipboard.writeText(md);
        showToast(`已导出 ${ids.length} 个会话到剪贴板`);
      }
    } catch (e) {
      showToast("批量导出失败: " + e);
    }
    exitMultiSelect();
  });

  document.getElementById("multi-cancel").addEventListener("click", exitMultiSelect);
  updateStatusBar();
}

function hideMultiBar() {
  const bar = document.getElementById("multi-bar");
  if (bar) bar.style.display = "none";
  updateStatusBar();
}

// ======================== Feature 7: 自动标签按钮 ========================

document.getElementById("auto-tag-btn").addEventListener("click", async () => {
  try {
    const count = await invoke("auto_tag_sessions");
    allTags = await invoke("get_all_tags");
    showToast(`自动标签完成，新增 ${count} 个会话标签`);
    render();
  } catch (e) {
    showToast("自动标签失败: " + e);
  }
});

// 窗口可见时刷新
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    searchInput.focus();
    loadSessions();
  }
});

init();
