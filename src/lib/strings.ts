/**
 * UI 文案集中管理。
 *
 * 浮窗界面（折叠态/展开态/仓库表/筛选/操作）的中文文案统一从这里取，
 * 组件不再内联字符串；这是 i18n 的前置结构（DESIGN_AND_BUILD_SPEC §8
 * 规格对齐后仍为中文优先）。设置页文案随后续重构分批迁入。
 */

import type { RemoteRelation } from "../types";

export const uiStrings = {
  app: {
    initialLoading: "正在刷新仓库状态...",
    loadFailedPrefix: "加载仓库失败",
    retry: "重试",
    openSettings: "打开设置",
  },
  collapsed: {
    brand: "GitaView",
    repoWord: "仓库",
    expandLabel: "展开仓库状态",
    hintDraggable: "点击展开，拖动移动，右键菜单",
    hint: "点击展开，右键菜单",
    stale: "数据未更新",
    stalePrefix: "数据未更新",
    lastRefreshPrefix: "上次刷新",
    contextMenuError: "打开收缩态右键菜单失败",
  },
  expanded: {
    title: "仓库状态",
    refreshTimePrefix: "刷新时间",
    notRefreshed: "尚未刷新",
    searchPlaceholder: "搜索或分组",
    searchLabel: "搜索仓库",
    refresh: "刷新",
    refreshing: "刷新中",
    refreshActionLabel: "刷新",
    openSettings: "打开设置",
    collapse: "收起",
    refreshFailedPrefix: "刷新失败",
    empty: "没有匹配的仓库",
    dragHint: "拖动空白区域移动窗口",
  },
  filters: {
    all: "全部",
  },
  relation: {
    error: "读取失败",
    synced: "已同步",
    local_ahead: "本地领先",
    remote_ahead: "远程领先",
    diverged: "分叉",
    no_remote: "无远端",
  } as const satisfies Record<RemoteRelation, string>,
  table: {
    status: "状态",
    name: "仓库",
    group: "分类",
    branch: "分支",
    relation: "关系",
    changes: "变更",
    hint: "提示",
    resizeHint: (label: string) => `调整${label}列宽，左右方向键步进`,
  },
  actions: {
    directory: "目录",
    remote: "远端",
    loading: "加载中...",
    fetch: "Fetch",
    push: "Push",
    pull: "Pull",
    confirmPush: "确认 Push",
    confirmPull: "确认 Pull",
    directoryTitle: "在文件管理器中打开仓库目录",
    remoteTitle: "在浏览器中打开远端仓库页面",
    remoteMissingTitle: "未配置远端仓库",
    fetchTitle: "从远端获取最新分支信息",
    pushTitle: "将本地提交推送到远端",
    pullTitle: "从远端拉取更新并合并到本地",
    pullWarning: "Pull 会修改当前仓库工作区，是否继续？",
    pushWarning: "Push 会更新远端分支，是否继续？",
  },
} as const;

export type UiStrings = typeof uiStrings;
