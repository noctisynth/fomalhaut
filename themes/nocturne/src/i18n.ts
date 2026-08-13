import type { UiLocale } from "fomalhaut-sdk";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";

const english = {
  "back.cancel-authentication": "Cancel authentication",
  "back.users": "Back to users",
  "loading.connecting": "Connecting to Fomalhaut…",
  "unavailable.title": "Fomalhaut unavailable",
  "selection.title": "Who’s signing in?",
  "selection.description": "Choose an account to continue on this device.",
  "selection.other-user": "Other user",
  "selection.manual-username": "Enter a username manually",
  "sign-in.title": "Sign in",
  "sign-in.description": "Enter a local or directory account.",
  "authentication.title": "Authentication",
  "authentication.recovery":
    "The host has an active sign-in without recoverable identity details.",
  "authentication.failure": "Authentication failed. Try again.",
  "authentication.waiting": "Waiting for the authentication service…",
  "lock.brand": "Fomalhaut Lock",
  "lock.acquiring": "Securing this session…",
  "lock.unlocking": "Unlocking…",
  "lock.released": "Session unlocked",
  "lock.failed": "The native session lock is unavailable",
  "form.username": "Username",
  "form.password": "Password",
  "form.continue": "Continue",
  "form.sign-in": "Sign in",
  "form.waiting": "Waiting for authentication…",
  "form.username-first": "Enter username first",
  "form.try-again": "Try again",
  "session.label": "Session",
  "session.choose": "Choose session",
  "power.menu": "Power menu",
  "power.actions": "Power actions",
  "power.poweroff": "Power off",
  "power.reboot": "Restart",
  "power.suspend": "Suspend",
  "power.question.poweroff": "Power off this device?",
  "power.question.reboot": "Restart this device?",
  "power.question.suspend": "Suspend this device?",
  "power.warning": "Active work in other sessions may be interrupted.",
  "power.cancel": "Cancel",
  "power.confirm.poweroff": "Confirm power off",
  "power.confirm.reboot": "Confirm restart",
  "power.confirm.suspend": "Confirm suspend",
  "error.busy": "Another authentication request is still in progress.",
  "error.host-unavailable": "The Fomalhaut host is unavailable.",
  "error.request-failed": "Fomalhaut could not complete the request.",
  "error.start-title": "Fomalhaut could not start",
  "error.theme-title": "Theme error",
  "error.theme-description":
    "Restart Fomalhaut or ask an administrator to inspect the host.",
} as const;

export type TranslationKey = keyof typeof english;
type Catalog = Record<TranslationKey, string>;

const simplifiedChinese = {
  "back.cancel-authentication": "取消认证",
  "back.users": "返回用户列表",
  "loading.connecting": "正在连接 Fomalhaut…",
  "unavailable.title": "Fomalhaut 不可用",
  "selection.title": "谁要登录？",
  "selection.description": "选择一个账户以继续使用此设备。",
  "selection.other-user": "其他用户",
  "selection.manual-username": "手动输入用户名",
  "sign-in.title": "登录",
  "sign-in.description": "输入本地账户或目录账户。",
  "authentication.title": "身份认证",
  "authentication.recovery": "宿主中存在活动登录，但无法恢复身份信息。",
  "authentication.failure": "认证失败，请重试。",
  "authentication.waiting": "正在等待认证服务…",
  "lock.brand": "Fomalhaut 锁屏",
  "lock.acquiring": "正在保护当前会话…",
  "lock.unlocking": "正在解锁…",
  "lock.released": "会话已解锁",
  "lock.failed": "原生会话锁不可用",
  "form.username": "用户名",
  "form.password": "密码",
  "form.continue": "继续",
  "form.sign-in": "登录",
  "form.waiting": "正在等待认证…",
  "form.username-first": "请先输入用户名",
  "form.try-again": "重试",
  "session.label": "会话",
  "session.choose": "选择会话",
  "power.menu": "电源菜单",
  "power.actions": "电源操作",
  "power.poweroff": "关机",
  "power.reboot": "重启",
  "power.suspend": "挂起",
  "power.question.poweroff": "要关闭此设备吗？",
  "power.question.reboot": "要重启此设备吗？",
  "power.question.suspend": "要挂起此设备吗？",
  "power.warning": "其他会话中未保存的工作可能会中断。",
  "power.cancel": "取消",
  "power.confirm.poweroff": "确认关机",
  "power.confirm.reboot": "确认重启",
  "power.confirm.suspend": "确认挂起",
  "error.busy": "另一个认证请求仍在处理中。",
  "error.host-unavailable": "Fomalhaut 宿主不可用。",
  "error.request-failed": "Fomalhaut 无法完成该请求。",
  "error.start-title": "Fomalhaut 无法启动",
  "error.theme-title": "主题错误",
  "error.theme-description": "请重启 Fomalhaut，或联系管理员检查宿主。",
} satisfies Catalog;

const catalogs = {
  en: { translation: english },
  "zh-CN": { translation: simplifiedChinese },
} as const satisfies Record<UiLocale, { translation: Catalog }>;

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    returnNull: false;
    resources: (typeof catalogs)["en"];
  }
}

void i18n.use(initReactI18next).init({
  resources: catalogs,
  lng: detectBrowserLocale(),
  fallbackLng: "en",
  supportedLngs: ["en", "zh-CN"],
  load: "currentOnly",
  initAsync: false,
  returnNull: false,
  interpolation: { escapeValue: false },
});

export function translate(locale: UiLocale, key: TranslationKey): string {
  return i18n.t(key, { lng: locale });
}

export { i18n };

export function detectBrowserLocale(
  languages: readonly string[] = typeof navigator === "undefined"
    ? []
    : navigator.languages,
): UiLocale {
  return languages.some((language) => {
    const normalized = language.trim().replaceAll("_", "-").toLowerCase();
    return normalized === "zh" || normalized.startsWith("zh-");
  })
    ? "zh-CN"
    : "en";
}
