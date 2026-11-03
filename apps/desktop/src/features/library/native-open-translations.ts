import i18n from "../../i18n";
import "../tools/navigation";

const values = {
  "en-US": {
    nativeOpen: "Open in system app",
    nativeClose: "Close selection",
    nativeRequested: "Submitted system opening request",
    nativeSelect: "Choose a version or Part to open",
    nativeNoFiles: "No supported local media file",
    detailsAction: "Details",
  },
  "zh-CN": {
    nativeOpen: "使用系统应用打开",
    nativeClose: "关闭选择",
    nativeRequested: "已提交系统打开请求",
    nativeSelect: "选择要打开的版本或 Part",
    nativeNoFiles: "没有支持的本地媒体文件",
    detailsAction: "查看详情",
  },
};
for (const [locale, bundle] of Object.entries(values))
  i18n.addResourceBundle(locale, "translation", bundle, true, true);
