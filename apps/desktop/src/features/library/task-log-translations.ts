import i18n from "../../i18n";

const values = {
  "zh-CN": {
    taskLog: "任务日志",
    taskLogEmpty: "暂无任务记录",
    taskLogAttention: "需处理",
    taskLogLibrary: "媒体库",
    taskLogRefresh: "更新媒体库 · {{name}}",
    taskLogScan: "扫描目录 · {{name}}",
    taskLogSearch: "{{name}} · 检索 {{query}}",
    taskLogSource: "资料来源",
    taskLogChecked: "已检查 {{count}} 个文件",
    taskLogCandidates: "发现 {{count}} 项候选",
    taskLogSkipped: "跳过 {{count}} 个文件",
    taskLogWarnings: "{{count}} 条提醒",
  },
  "en-US": {
    taskLog: "Task log",
    taskLogEmpty: "No task records yet",
    taskLogAttention: "Needs attention",
    taskLogLibrary: "Library",
    taskLogRefresh: "Refresh library · {{name}}",
    taskLogScan: "Scan folder · {{name}}",
    taskLogSearch: "{{name}} · Search for {{query}}",
    taskLogSource: "Metadata source",
    taskLogChecked: "{{count}} files checked",
    taskLogCandidates: "{{count}} candidates found",
    taskLogSkipped: "{{count}} files skipped",
    taskLogWarnings: "{{count}} warnings",
  },
};

for (const [language, translations] of Object.entries(values))
  i18n.addResourceBundle(language, "translation", translations, true, true);
