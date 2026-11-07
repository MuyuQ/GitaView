#include "gv_platform.h"

#include <appmodel.h>
#include <shlobj.h>
#include <shcore.h>

#include <string>

#include "resource.h"

namespace gitaview {
namespace {

constexpr UINT kTrayCallbackMessage = WM_APP + 1;
constexpr int kTrayIconId = 1;
// 右键菜单项（与 C# TrayWindow 菜单一致：打开主程序 / 刷新 / 退出）。
constexpr int kMenuOpenMain = 1;
constexpr int kMenuRefresh = 2;
constexpr int kMenuExit = 3;

std::wstring ReadEnvironmentPath(const wchar_t* name) {
  DWORD size = GetEnvironmentVariableW(name, nullptr, 0);
  if (size == 0) return L"";
  std::wstring value(size, L'\0');
  size = GetEnvironmentVariableW(name, value.data(), size);
  value.resize(size);
  return value;
}

// 打包身份 → LocalState（与 C# ApplicationData.Current.LocalFolder 等价）；
// 未打包（开发运行）→ %LOCALAPPDATA%\GitaView。
std::wstring ResolveLocalStateBase() {
  WCHAR family[PACKAGE_FAMILY_NAME_MAX_LENGTH] = {};
  UINT32 length = PACKAGE_FAMILY_NAME_MAX_LENGTH;
  if (GetCurrentPackageFamilyName(&length, family) == ERROR_SUCCESS) {
    std::wstring base = ReadEnvironmentPath(L"LOCALAPPDATA");
    if (!base.empty()) {
      std::wstring path = base + L"\\Packages\\" + family + L"\\LocalState";
      SHCreateDirectoryExW(nullptr, path.c_str(), nullptr);
      return path;
    }
  }
  std::wstring base = ReadEnvironmentPath(L"LOCALAPPDATA");
  std::wstring path = base + L"\\GitaView";
  SHCreateDirectoryExW(nullptr, path.c_str(), nullptr);
  return path;
}

// StartupTask 注册表镜像：SystemAppData\<PFN>\GitaViewStartup\State。
// 值含义未在公开文档稳定承诺：原样返回整数（-1 = 读不到），映射在 Dart 侧。
int64_t ReadStartupTaskState() {
  WCHAR family[PACKAGE_FAMILY_NAME_MAX_LENGTH] = {};
  UINT32 length = PACKAGE_FAMILY_NAME_MAX_LENGTH;
  if (GetCurrentPackageFamilyName(&length, family) != ERROR_SUCCESS) {
    return -1;
  }
  std::wstring sub_key =
      L"Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\"
      L"CurrentVersion\\AppModel\\SystemAppData\\";
  sub_key += family;
  sub_key += L"\\GitaViewStartup";
  HKEY key = nullptr;
  if (RegOpenKeyExW(HKEY_CURRENT_USER, sub_key.c_str(), 0, KEY_READ, &key) !=
      ERROR_SUCCESS) {
    return -1;
  }
  DWORD value = 0;
  DWORD size = sizeof(value);
  DWORD type = 0;
  const LRESULT result =
      RegGetValueW(key, nullptr, L"State", RRF_RT_REG_DWORD, &type, &value, &size);
  RegCloseKey(key);
  if (result != ERROR_SUCCESS) {
    return -1;
  }
  return static_cast<int64_t>(value);
}

}  // namespace

GvPlatform* GvPlatform::instance_ = nullptr;

void GvPlatform::Register(flutter::PluginRegistrarWindows* registrar) {
  // 同一进程只允许一个实例（托盘窗口与图标唯一）。
  if (instance_ != nullptr) return;
  instance_ = new GvPlatform(registrar);
}

GvPlatform::GvPlatform(flutter::PluginRegistrarWindows* registrar)
    : registrar_(registrar) {
  channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
      registrar->messenger(), "gitaview/platform",
      &flutter::StandardMethodCodec::GetInstance());
  channel_->SetMethodCallHandler(
      [this](const auto& call, auto result) { HandleMethodCall(call, std::move(result)); });
  taskbar_created_message_ = RegisterWindowMessageW(L"TaskbarCreated");

  WNDCLASSW window_class = {};
  window_class.lpfnWndProc = GvPlatform::TrayWndProc;
  window_class.hInstance = GetModuleHandleW(nullptr);
  window_class.lpszClassName = L"GitaViewTrayWindow";
  RegisterClassW(&window_class);

  tray_window_ = CreateWindowExW(0, L"GitaViewTrayWindow", L"GitaView tray", 0,
                                 0, 0, 0, 0, HWND_MESSAGE, nullptr,
                                 GetModuleHandleW(nullptr), nullptr);
}

bool GvPlatform::EnsureTray() {
  if (tray_window_ == nullptr) return false;
  if (tray_added_) return true;
  HICON icon = static_cast<HICON>(LoadImageW(
      GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDI_APP_ICON), IMAGE_ICON,
      GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON),
      LR_DEFAULTCOLOR));
  if (icon == nullptr) {
    icon = LoadIconW(nullptr, IDI_APPLICATION);
  }
  notify_icon_ = {};
  notify_icon_.cbSize = sizeof(notify_icon_);
  notify_icon_.hWnd = tray_window_;
  notify_icon_.uID = kTrayIconId;
  notify_icon_.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
  notify_icon_.uCallbackMessage = kTrayCallbackMessage;
  notify_icon_.hIcon = icon;
  lstrcpynW(notify_icon_.szTip, L"GitaView", ARRAYSIZE(notify_icon_.szTip));
  tray_added_ = Shell_NotifyIconW(NIM_ADD, &notify_icon_) != FALSE;
  return tray_added_;
}

void GvPlatform::RemoveTray() {
  if (tray_added_) {
    Shell_NotifyIconW(NIM_DELETE, &notify_icon_);
    tray_added_ = false;
  }
}

bool GvPlatform::GetTrayIconRect(RECT* rect) const {
  if (!tray_added_) return false;
  // Shell_NotifyIconGetRect 接收 NOTIFYICONIDENTIFIER（与 NOTIFYICONDATAW
  // 前三字段同形但类型不同，不能直接传结构体）。
  NOTIFYICONIDENTIFIER identifier = {};
  identifier.cbSize = sizeof(identifier);
  identifier.hWnd = tray_window_;
  identifier.uID = kTrayIconId;
  return Shell_NotifyIconGetRect(&identifier, rect) == S_OK;
}

void GvPlatform::SendTrayContext(
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result)
    const {
  RECT icon = {};
  const bool has_icon = GetTrayIconRect(&icon);
  // 托盘图标所在显示器的工作区（物理像素）与 DPI 缩放。
  POINT center = {icon.left + (icon.right - icon.left) / 2,
                  icon.top + (icon.bottom - icon.top) / 2};
  HMONITOR monitor =
      MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
  MONITORINFO monitor_info = {};
  monitor_info.cbSize = sizeof(monitor_info);
  const bool has_monitor =
      monitor != nullptr && GetMonitorInfoW(monitor, &monitor_info) != FALSE;
  UINT dpi = 96;
  if (monitor != nullptr) {
    GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &dpi, &dpi);
  }
  const RECT& work = has_monitor ? monitor_info.rcWork : icon;
  flutter::EncodableMap response = {
      {"icon",
       flutter::EncodableValue(flutter::EncodableMap{
           {"x", flutter::EncodableValue(int64_t{icon.left})},
           {"y", flutter::EncodableValue(int64_t{icon.top})},
           {"width", flutter::EncodableValue(int64_t{icon.right - icon.left})},
           {"height",
            flutter::EncodableValue(int64_t{icon.bottom - icon.top})}})},
      {"workArea",
       flutter::EncodableValue(flutter::EncodableMap{
           {"x", flutter::EncodableValue(int64_t{work.left})},
           {"y", flutter::EncodableValue(int64_t{work.top})},
           {"width", flutter::EncodableValue(int64_t{work.right - work.left})},
           {"height",
            flutter::EncodableValue(int64_t{work.bottom - work.top})}})},
      {"scale", flutter::EncodableValue(double{dpi} / 96.0)},
      {"hasIcon", flutter::EncodableValue(has_icon)},
  };
  result->Success(flutter::EncodableValue(response));
}

void GvPlatform::ShowContextMenu() {
  POINT point = {};
  GetCursorPos(&point);
  // TrackPopupMenu 前置前台 + 之后补 WM_NULL：托盘菜单焦点规则（KB135788，
  // 与 C# TrayWindow 相同），否则菜单不随外点关闭。
  SetForegroundWindow(tray_window_);
  HMENU menu = CreatePopupMenu();
  AppendMenuW(menu, MF_STRING, kMenuOpenMain, L"打开 GitaView");
  AppendMenuW(menu, MF_STRING, kMenuRefresh, L"刷新");
  AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
  AppendMenuW(menu, MF_STRING, kMenuExit, L"退出");
  SetMenuDefaultItem(menu, kMenuOpenMain, FALSE);
  const int command = TrackPopupMenu(
      menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, point.x, point.y,
      0, tray_window_, nullptr);
  DestroyMenu(menu);
  PostMessageW(tray_window_, WM_NULL, 0, 0);
  if (command != 0) {
    NotifyDart("onTrayMenuAction",
               std::make_unique<flutter::EncodableValue>(int64_t{command}));
  }
}

LRESULT CALLBACK GvPlatform::TrayWndProc(HWND window, UINT message,
                                         WPARAM wparam, LPARAM lparam) {
  if (instance_ != nullptr) {
    instance_->OnTrayMessage(window, message, wparam, lparam);
  }
  return DefWindowProcW(window, message, wparam, lparam);
}

void GvPlatform::OnTrayMessage(HWND window, UINT message, WPARAM wparam,
                               LPARAM lparam) {
  if (message == taskbar_created_message_ && taskbar_created_message_ != 0) {
    // Explorer 重启：托盘图标随任务栏重建，必须重新注册。
    tray_added_ = false;
    EnsureTray();
    return;
  }
  if (message != kTrayCallbackMessage) return;
  if (LOWORD(lparam) == WM_LBUTTONUP) {
    // 左键：打开 / 收起浮窗由 Dart 决定（与 C# TrayWindow 点击行为一致）。
    NotifyDart("onTrayLeftClick", nullptr);
  } else if (LOWORD(lparam) == WM_RBUTTONUP || LOWORD(lparam) == WM_CONTEXTMENU) {
    ShowContextMenu();
  }
}

void GvPlatform::NotifyDart(
    const char* method,
    std::unique_ptr<flutter::EncodableValue> argument) const {
  channel_->InvokeMethod(method, std::move(argument));
}

void GvPlatform::HandleMethodCall(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  const std::string& method = call.method_name();
  if (method == "getSystemPaths") {
    const std::wstring base = ResolveLocalStateBase();
    flutter::EncodableMap response = {
        {"dataDirectory", flutter::EncodableValue(base + L"\\data")},
        {"logDirectory", flutter::EncodableValue(base + L"\\logs")},
    };
    result->Success(flutter::EncodableValue(response));
    return;
  }
  if (method == "ensureTray") {
    result->Success(flutter::EncodableValue(EnsureTray()));
    return;
  }
  if (method == "removeTray") {
    RemoveTray();
    result->Success();
    return;
  }
  if (method == "getTrayContext") {
    SendTrayContext(std::move(result));
    return;
  }
  if (method == "getStartupTaskState") {
    result->Success(flutter::EncodableValue(int64_t{ReadStartupTaskState()}));
    return;
  }
  if (method == "openTarget") {
    // 打开目录（Explorer）或远端 URL（默认浏览器）。核心 repoTarget 已做
    // 安全复核；此处仍只放行 http/https URL，双保险。
    bool ok = false;
    if (const auto args = std::get_if<flutter::EncodableMap>(call.arguments())) {
      const auto kind = args->find(flutter::EncodableValue("kind"));
      const auto value = args->find(flutter::EncodableValue("value"));
      if (kind != args->end() && value != args->end() &&
          std::holds_alternative<std::string>(value->second) &&
          std::holds_alternative<std::string>(kind->second)) {
        const auto& kind_string = std::get<std::string>(kind->second);
        const auto& target = std::get<std::string>(value->second);
        const bool allowed = kind_string == "directory" ||
                             (kind_string == "remote" &&
                              (target.rfind("http://", 0) == 0 ||
                               target.rfind("https://", 0) == 0));
        if (allowed) {
          // UTF-8 → UTF-16：路径与 URL 可能包含非 ASCII（中文目录名）。
          const int wide_length = MultiByteToWideChar(
              CP_UTF8, 0, target.c_str(), static_cast<int>(target.size()),
              nullptr, 0);
          std::wstring wide(static_cast<size_t>(wide_length), L'\0');
          MultiByteToWideChar(CP_UTF8, 0, target.c_str(),
                              static_cast<int>(target.size()), wide.data(),
                              wide_length);
          // ShellExecuteW 失败返回 SE_ERR_*（≤32 的伪句柄），只有 >32 算成功。
          const auto executed = reinterpret_cast<INT_PTR>(ShellExecuteW(
              nullptr, L"open", wide.c_str(), nullptr, nullptr, SW_SHOWNORMAL));
          ok = executed > 32;
        }
      }
    }
    result->Success(flutter::EncodableValue(ok));
    return;
  }
  result->NotImplemented();
}

}  // namespace gitaview
