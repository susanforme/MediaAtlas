import i18n from "../../i18n";

const en = {
  error_maintenance_privacy_locked:
    "Unlock the adult workspace before full-library backup or restore.",
  error_maintenance_busy:
    "Another operation or application instance is active. Try again when it has stopped.",
  error_maintenance_conflict:
    "The preview or recovery files changed. Create a fresh preview.",
  error_maintenance_corrupt_backup:
    "The backup failed integrity or checksum validation. Preserved files are retained.",
  error_maintenance_unsupported_version:
    "This backup requires a newer application version.",
  error_maintenance_cancelled: "Maintenance was cancelled.",
  error_maintenance_recovery_required:
    "An interrupted restore needs attention. Keep all recovery files; no files were overwritten.",
  maintenanceTitle: "Backup and maintenance",
  maintenanceHelp:
    "Backups contain the complete library database, including local paths, history and private metadata. Source media files are not traversed or copied. The database may contain historical tag recovery bytes; credential values are excluded. Keep backups private.",
  maintenanceDirectory: "Backup and diagnostic folder",
  maintenanceBackupPreview: "Preview full database backup",
  maintenanceBackupConfirm: "Confirm database backup",
  maintenanceBackupWarning:
    "This snapshot includes all library metadata. It does not read source media or include OS/session secrets. The database may contain historical tag recovery bytes. Private metadata requires the adult workspace to be unlocked.",
  maintenancePrivate: "Includes private metadata",
  maintenanceSaved: "Backup saved.",
  maintenanceRestorePreview: "Preview restore",
  maintenanceRestoreConfirm: "Confirm restore at next startup",
  maintenanceRestoreWarning:
    "Restoration replaces the current library database at the next startup. A safety snapshot of the current database is preserved first. Media files are unchanged. Credentials may need to be configured again. Fully quit, then reopen the app after confirming.",
  maintenanceQueued:
    "Restore queued. Fully quit and reopen MediaAtlas. Current changes remain in the safety snapshot.",
  maintenancePending: "A restore is queued for the next startup.",
  maintenanceNoBackups: "No database backups yet.",
  maintenanceDiagnostics: "Export safe diagnostics",
  maintenanceDiagnosticsHelp:
    "Exports only application/schema versions, fixed error codes and aggregate statistics. It excludes raw logs, local paths, URLs, queries, tokens and titles.",
  maintenanceExported: "Diagnostics saved",
  maintenanceCachePreview: "Preview artwork cache reset",
  maintenanceCacheConfirm: "Confirm cache reset",
  maintenanceCacheHelp:
    "Moves ordinary artwork cache files to a private maintenance quarantine. Original bytes are retained; this resets the cache without reclaiming disk space. Media, backups and private artwork are retained.",
  maintenanceCacheCount: "{{files}} cached files · {{bytes}} bytes",
  maintenanceCacheCleared:
    "Moved {{files}} files out of the active cache; quarantine bytes are retained.",
  maintenanceCancel: "Cancel operation",
  maintenanceDismiss: "Discard preview",
  maintenanceWorking: "Maintenance in progress…",
  maintenanceRefresh: "Refresh maintenance",
  maintenanceManualUpdate:
    "Updates are manual for this beta. Download a matching native package from the official Release, verify its SHA-256, quit MediaAtlas and install it. Application data is retained. Automatic updates and signed/notarized packages are not configured.",
};
const zh: Record<keyof typeof en, string> = {
  error_maintenance_privacy_locked:
    "完整媒体库备份或恢复前，请先解锁成人工作区。",
  error_maintenance_busy: "另一个操作或应用实例正在运行，请等待其停止后重试。",
  error_maintenance_conflict: "预览或恢复文件已变化，请重新预览。",
  error_maintenance_corrupt_backup:
    "备份完整性或校验和验证失败，恢复文件已保留。",
  error_maintenance_unsupported_version: "此备份需要更高版本的应用。",
  error_maintenance_cancelled: "维护已取消。",
  error_maintenance_recovery_required:
    "中断的恢复需要处理，请保留所有恢复文件；未覆盖任何文件。",
  maintenanceTitle: "备份与维护",
  maintenanceHelp:
    "完整备份包含全部媒体库数据库，包括本地路径、历史记录及隐私元数据，不遍历或复制源媒体文件；数据库可能含历史标签恢复字节，不含凭据值。请妥善保管备份。",
  maintenanceDirectory: "备份与诊断目录",
  maintenanceBackupPreview: "预览完整数据库备份",
  maintenanceBackupConfirm: "确认备份数据库",
  maintenanceBackupWarning:
    "快照含全部媒体库元数据，不读取源媒体或包含系统／会话秘密，但数据库可能含历史标签恢复字节。含隐私元数据时须先解锁成人工作区。",
  maintenancePrivate: "含隐私元数据",
  maintenanceSaved: "备份已保存。",
  maintenanceRestorePreview: "预览恢复",
  maintenanceRestoreConfirm: "确认下次启动恢复",
  maintenanceRestoreWarning:
    "恢复将在下次启动替换当前媒体库数据库，替换前先保留当前数据库的安全快照。媒体文件不变，凭据可能需要重新配置。确认后请完全退出应用再重新打开。",
  maintenanceQueued:
    "已排定恢复。请完全退出并重新打开 MediaAtlas，当前更改会保留在安全快照中。",
  maintenancePending: "已有恢复计划，将在下次启动应用。",
  maintenanceNoBackups: "暂无数据库备份。",
  maintenanceDiagnostics: "导出安全诊断",
  maintenanceDiagnosticsHelp:
    "仅导出应用／数据库版本、固定错误码及汇总统计，不含原始日志、本地路径、URL、查询、令牌或标题。",
  maintenanceExported: "诊断已保存",
  maintenanceCachePreview: "预览图片缓存重置",
  maintenanceCacheConfirm: "确认重置缓存",
  maintenanceCacheHelp:
    "将普通图片缓存移至维护隔离区，保留原始字节；本操作重置缓存，不释放磁盘空间。源媒体、备份及隐私图片保留。",
  maintenanceCacheCount: "{{files}} 个缓存文件 · {{bytes}} 字节",
  maintenanceCacheCleared:
    "已移出 {{files}} 个活跃缓存文件；原始字节保留在隔离区。",
  maintenanceCancel: "取消操作",
  maintenanceDismiss: "放弃预览",
  maintenanceWorking: "维护处理中…",
  maintenanceRefresh: "刷新维护信息",
  maintenanceManualUpdate:
    "此 beta 采用手动更新。从官方 Release 下载匹配平台的原生包并验证 SHA-256，退出 MediaAtlas 后安装。应用数据会保留。尚未配置自动更新或签名／公证包。",
};
i18n.addResourceBundle("en-US", "translation", en, true, true);
i18n.addResourceBundle("zh-CN", "translation", zh, true, true);
