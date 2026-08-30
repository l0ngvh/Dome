-- Bundled default ignore rules for Windows. Appended to the user's ignore list.
return {
  { process = "LockApp.exe" },
  { process = "SearchHost.exe" },
  { process = "StartMenuExperienceHost.exe" },
  { title = "MSCTFIME UI" },   -- IME hidden window
  { title = "OLEChannelWnd" }, -- OLE hidden window
  { class = "Shell_TrayWnd" },
  { class = "Shell_SecondaryTrayWnd" }, -- secondary-monitor taskbar
  { class = "Progman" },                -- desktop window
  { class = "WorkerW" },                -- desktop wallpaper host
  { class = "TaskListThumbnailWnd" },
  { class = "MultitaskingViewFrame" },   -- Task View
  { class = "Xaml_WindowedPopupClass" }, -- XAML popup surface
  { class = "TaskManagerWindow" },
  { class = "Windows.UI.Core.CoreWindow" }, -- UWP core window
  { class = [[/^MessageWindowClass\+/]] },
}
