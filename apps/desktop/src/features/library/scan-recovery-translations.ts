import i18n from "../../i18n";

const values = {
  "zh-CN": {
    scanRecoveryRoot:
      "目录不可用。请检查文件夹是否仍存在、磁盘或网络共享是否已连接，再到目录管理中确认位置。",
    scanRecoveryExclusions:
      "排除规则无效。请在目录管理中编辑该目录的高级设置，修正规则后再更新。",
    scanRecoveryUnreadable:
      "本地文件无法读取。请检查目录访问权限及磁盘或网络连接，再更新目录。",
    scanRecoveryUnsafeMetadata:
      "本地 NFO/XML 文件无法安全解析。请检查该文件，修正后再更新目录。",
    scanRecoveryInvalidMetadata:
      "本地资料文件格式无效。请检查 NFO/XML 内容，修正后再更新目录。",
    scanRecoveryLargeMetadata:
      "本地资料文件超过读取限制。请检查 NFO/XML 文件是否异常过大，再更新目录。",
    scanRecoveryChanged:
      "扫描期间部分文件发生变化。这些文件尚未收录，已收录内容保留；文件稳定后可重新更新目录。",
    scanRecoveryFailed:
      "目录更新未完成，已收录内容保留。可以重新更新目录；若仍失败，请检查目录和来源配置。",
    scanRecoveryCancelled:
      "更新已停止，已收录内容和已保存资料保留。再次更新目录可继续处理尚未完成的内容。",
    scanRecoveryInterrupted:
      "更新意外中断，已收录内容和已保存资料保留。可重新更新目录。",
    scanRecoveryManage: "管理目录",
    scanRecoveryRetry: "重新更新目录",
    scanRecoveryViewRetry: "查看并重新更新",
  },
  "en-US": {
    scanRecoveryRoot:
      "The directory is unavailable. Check that the folder exists and its disk or network share is connected, then confirm its location in directory management.",
    scanRecoveryExclusions:
      "An exclude rule is invalid. Edit this directory's advanced settings in directory management, then update it again.",
    scanRecoveryUnreadable:
      "A local file could not be read. Check directory permissions and the disk or network connection, then update the directory again.",
    scanRecoveryUnsafeMetadata:
      "A local NFO/XML file could not be parsed safely. Check and correct the file before updating the directory again.",
    scanRecoveryInvalidMetadata:
      "A local metadata file has an invalid format. Check and correct its NFO/XML content, then update the directory again.",
    scanRecoveryLargeMetadata:
      "A local metadata file exceeds the reading limit. Check for an unexpectedly large NFO/XML file before updating the directory again.",
    scanRecoveryChanged:
      "Some files changed during the scan and have not been added. Already indexed content is retained; update the directory again once the files are stable.",
    scanRecoveryFailed:
      "The directory update did not finish. Already indexed content is retained. Update the directory again; if it still fails, check the directory and source configuration.",
    scanRecoveryCancelled:
      "The update stopped. Already indexed content and saved metadata are retained. Update the directory again to process unfinished items.",
    scanRecoveryInterrupted:
      "The update was interrupted. Already indexed content and saved metadata are retained. You can update the directory again.",
    scanRecoveryManage: "Manage directories",
    scanRecoveryRetry: "Update directory again",
    scanRecoveryViewRetry: "View and update again",
  },
};
for (const [language, translations] of Object.entries(values))
  i18n.addResourceBundle(language, "translation", translations, true, true);
