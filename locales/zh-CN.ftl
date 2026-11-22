# Shared titles
title-update = { $app_title } 更新
title-setup = { $app_title } 安装
title-uninstall = { $app_title } 卸载
error-title = 安装程序

# Shared buttons
btn-cancel = 取消
btn-install-update = 安装更新
btn-install = 安装
btn-update = 更新
btn-downgrade = 安装旧版本
btn-repair = 修复
btn-open-log = 查看日志
btn-open-install-dir = 打开安装位置
btn-ok = 确定
# Elevation (dialogs_common.rs)
elevate-header = 更新 { $app_title } 需要管理员权限
elevate-body = 更新到 { $app_version } 需要管理员权限。是否继续？

# Restart required (prerequisite.rs)
restart-header = 安装 { $app_title } 需要重新启动电脑
restart-body = 请在重新启动电脑后再次运行安装程序。

# Missing dependencies (prerequisite.rs)
missing-deps-header = { $app_title } 需要其他组件
missing-deps-body = 运行 { $app_title } 需要以下组件：{ $deps }。是否现在下载并安装？

# Uninstall with errors (uninstall)
uninstall-errors-header = { $app_title } 已卸载，部分文件未能移除
uninstall-errors-body = 有文件或文件夹仍被占用，未能移除。可以在关闭相关程序后手动删除。
uninstall-errors-log = 详细信息已保存至：{ $path }

# Overwrite/repair dialog (install.rs)
overwrite-already-installed = 已安装 { $app_title }
overwrite-repair-body = 这台电脑上已安装 { $app_title } { $app_version }，无需使用此安装程序进行更新。若无法正常使用 { $app_title }，可以修复当前安装。
overwrite-older-installed = 已安装 { $app_title }
overwrite-update-body = 这台电脑上已安装 { $old_version }。是否更新到 { $app_version }？
overwrite-newer-installed = 已安装较新版本的 { $app_title }
overwrite-downgrade-body = 这台电脑上已安装 { $app_title } { $old_version }，无需使用此安装程序进行更新。若降级安装旧版本将可能导致程序无法正常工作甚至损坏现有用户数据，是否确认继续？
overwrite-footer = 安装位置：{ $path }

# Uninstall complete (uninstall.rs)
uninstall-header = { $app_title } 已卸载
uninstall-body = 应用已从这台电脑移除。

# Uninstall progress (uninstall.rs)
uninstall-progress-header = 正在卸载 { $app_title }
uninstall-progress-body = 正在移除文件和快捷方式…

# Install hook failed (install.rs)
install-hook-header = { $app_title } 已安装，部分步骤未成功
install-hook-body = 如果应用无法正常使用，请重新安装。

# Splash fallback (splash.rs)
splash-header = 正在安装 { $app_title }
splash-body = 正在安装 { $app_title } { $app_version }，请稍候…

# Dependency download (prerequisite.rs)
deps-download-header = 正在下载 { $app_title } 所需的组件
deps-download-body = 正在下载 { $dep_name }…

# Apply progress (apply_*_impl.rs)
apply-header = 正在更新 { $app_title }
apply-body = 正在更新到 { $app_version }，请稍候…

# Start error (start_windows_impl.rs)
start-corrupt-header = 未能启动 { $app_title }
start-corrupt-body = 部分文件缺失或无法读取。请重新安装以恢复。

# Generic error
error-header = 未能完成操作

# Setup error (wix msi)
setup-error-header = 未能安装 { $app_title }
setup-disk-space-insufficient = 安装 { $app_title } 需要 { $required_space } 的可用空间，当前仅有 { $available_space }。请释放磁盘空间后重试。
setup-windows-version-unsupported = 此安装程序需要 Windows 7 SP1 或更高版本。
setup-embedded-zip-missing = 安装程序不完整，缺少安装包。请重新下载安装程序。
setup-os-version-required = 此应用需要 Windows { $os_version } 或更高版本。
setup-cpu-arch-unsupported = 此应用（{ $machine_arch }）不支持这台电脑的处理器架构。
setup-stop-app-failed = 未能关闭正在运行的应用（{ $error }）。请手动关闭后重新运行安装程序。
setup-remove-dir-failed = 未能移除现有的安装目录。请关闭应用后重新运行安装程序；如果问题仍然存在，请先在系统设置中卸载，或重新启动电脑。
setup-update-exe-missing = 安装程序不完整，缺少更新程序（Update.exe）。请重新下载安装程序。
setup-main-exe-missing = 安装包不完整，缺少主程序。请重新下载安装程序。

# MSI Installer UI - Common
msi-dlg-title = { $app_title } 安装程序
msi-btn-back = 上一步(&B)
msi-btn-next = 下一步(&N)
msi-btn-cancel = 取消
msi-btn-finish = 完成(&F)
msi-btn-ok = 确定
msi-btn-yes = 是(&Y)
msi-btn-no = 否(&N)
msi-btn-retry = 重试(&R)
msi-btn-ignore = 忽略(&I)

# MSI Installer UI - Welcome Dialog
msi-welcome-title = 欢迎使用 { $app_title } 安装向导
msi-welcome-description = 安装向导将在您的计算机上安装 { $app_title }。单击“下一步”继续，或单击“取消”退出安装向导。
msi-welcome-update-description = 安装向导将更新您计算机上的 { $app_title }。单击“下一步”继续，或单击“取消”退出安装向导。

# MSI Installer UI - Exit Dialog
msi-exit-title = { $app_title } 安装向导已完成
msi-exit-description = 单击“完成”按钮退出安装向导。
msi-exit-launch-checkbox = 启动 { $app_title }

# MSI Installer UI - Prepare Dialog
msi-prepare-title = 欢迎使用 { $app_title } 安装向导
msi-prepare-description = 安装向导正准备指导您完成安装过程，请稍候。

# MSI Installer UI - License Agreement Dialog
msi-license-title = 最终用户许可协议
msi-license-description = 请认真阅读以下许可协议。
msi-license-checkbox = 我接受许可协议中的条款(&A)

# MSI Installer UI - Install Scope Dialog
msi-scope-title = 安装范围
msi-scope-description = 选择安装范围。
msi-scope-per-user = 仅为您安装(&Y)
msi-scope-per-machine = 为所有用户安装(&A)
msi-scope-per-user-description = 仅为当前用户安装
msi-scope-no-per-user-description = 需要管理员权限
msi-scope-per-machine-description = 需要管理员权限

# MSI Installer UI - Verify Ready Dialog
msi-ready-install-title = 已准备好安装 { $app_title }
msi-ready-install-text = 单击“安装”开始安装。单击“上一步”查看或更改任何安装设置。
msi-ready-change-title = 已准备好更改 { $app_title }
msi-ready-change-text = 单击“更改”开始更改安装。单击“上一步”查看或更改任何安装设置。
msi-ready-repair-title = 已准备好修复 { $app_title }
msi-ready-repair-text = 单击“修复”开始修复。单击“上一步”查看或更改任何安装设置。
msi-ready-remove-title = 已准备好删除 { $app_title }
msi-ready-remove-text = 单击“删除”可从计算机上删除 { $app_title }。单击“上一步”查看或更改任何安装设置。
msi-ready-update-title = 已准备好更新 { $app_title }
msi-ready-update-text = 单击“更新”开始更新。单击“上一步”查看或更改任何安装设置。
msi-ready-btn-install = 安装(&I)
msi-ready-btn-change = 更改(&C)
msi-ready-btn-repair = 修复(&R)
msi-ready-btn-remove = 删除(&R)
msi-ready-btn-update = 更新(&U)

# MSI Installer UI - Progress Dialog
msi-progress-installing-title = 正在安装 { $app_title }
msi-progress-installing-text = 安装向导正在安装 { $app_title }，请稍候。
msi-progress-changing-title = 正在更改 { $app_title }
msi-progress-changing-text = 安装向导正在更改 { $app_title }，请稍候。
msi-progress-repairing-title = 正在修复 { $app_title }
msi-progress-repairing-text = 安装向导正在修复 { $app_title }，请稍候。
msi-progress-removing-title = 正在删除 { $app_title }
msi-progress-removing-text = 安装向导正在删除 { $app_title }，请稍候。
msi-progress-updating-title = 正在更新 { $app_title }
msi-progress-updating-text = 安装向导正在更新 { $app_title }，请稍候。
msi-progress-status = 状态:

# MSI Installer UI - Maintenance Welcome Dialog
msi-maint-welcome-title = 欢迎使用 { $app_title } 安装向导
msi-maint-welcome-description = 安装向导允许您修复或删除 { $app_title }。单击“下一步”继续，或单击“取消”退出安装向导。

# MSI Installer UI - Maintenance Type Dialog
msi-maint-type-title = 更改、修复或删除安装
msi-maint-type-description = 选择希望执行的操作。
msi-maint-change-button = 更改(&C)...
msi-maint-change-tooltip = 更改...
msi-maint-change-text = 允许用户更改要安装的程序功能以及更改单个功能。
msi-maint-change-disabled = 更改当前已禁用。
msi-maint-repair-button = 修复(&R)
msi-maint-repair-tooltip = 修复
msi-maint-repair-text = 修复最近安装中的错误 - 修复丢失或损坏的文件、快捷方式和注册表项。
msi-maint-repair-disabled = 修复当前已禁用。
msi-maint-remove-button = 删除(&M)
msi-maint-remove-tooltip = 删除
msi-maint-remove-text = 从您的计算机中删除 { $app_title }。
msi-maint-remove-disabled = 删除当前已禁用。

# MSI Installer UI - Cancel Dialog
msi-cancel-text = 是否确实要取消安装 { $app_title }?

# MSI Installer UI - Browse Dialog
msi-browse-title = 更改当前目标文件夹
msi-browse-description = 浏览到目标文件夹。
msi-browse-combo-label = 查找范围(&L):
msi-browse-path-label = 文件夹名称(&D):
msi-browse-up-tooltip = 向上一级
msi-browse-new-folder-tooltip = 新建文件夹

# MSI Installer UI - Invalid Directory Dialog
msi-invalid-dir-text = 指定的目标目录无效或位于不支持的驱动器类型上。

# MSI Installer UI - Disk Cost Dialog
msi-disk-cost-title = 磁盘空间要求
msi-disk-cost-description = 安装所选功能所需的磁盘空间。
msi-disk-cost-text = 突出显示的卷没有足够的磁盘空间用于当前所选功能。您可以从突出显示的卷中删除一些文件，安装较少的功能，或者选择其他目标驱动器。

# MSI Installer UI - Error Dialog
msi-error-dlg-title = { $app_title } 安装程序信息

# MSI Installer UI - Fatal Error Dialog
msi-fatal-title = { $app_title } 安装向导提前结束
msi-fatal-description1 = { $app_title } 安装程序已中断。您的系统尚未修改。若要稍后安装该程序，请再次运行安装程序。
msi-fatal-description2 = 单击“完成”按钮退出安装向导。

# MSI Installer UI - User Exit Dialog
msi-user-exit-title = { $app_title } 安装向导已中断
msi-user-exit-description1 = { $app_title } 安装程序已中断。您的系统尚未修改。若要稍后安装该程序，请再次运行安装程序。
msi-user-exit-description2 = 单击“完成”按钮退出安装向导。

# MSI Installer UI - Files In Use Dialog
msi-files-in-use-title = 使用中的文件
msi-files-in-use-description = 一些需要更新的文件当前正在使用中。
msi-files-in-use-text = 以下应用程序正在使用需要通过此安装程序更新的文件。请关闭这些应用程序，然后单击“重试”继续安装，或单击“取消”退出安装。
msi-files-in-use-exit = 退出(&X)

# MSI Installer UI - Restart Manager Files In Use Dialog
msi-rm-files-in-use-title = 使用中的文件
msi-rm-files-in-use-description = 一些需要更新的文件当前正在使用中。
msi-rm-files-in-use-text = 以下应用程序正在使用此安装程序需要更新的文件。可以让安装向导自动关闭并尝试重新启动这些应用程序，也可以手动关闭它们后单击“确定”继续安装。
msi-rm-files-in-use-use-rm = 自动关闭应用程序(&C)，并在安装完成后尝试重新启动它们。
msi-rm-files-in-use-dont-use-rm = 不关闭应用程序(&D)。(需要重新启动。)

# MSI Installer UI - Resume Dialog
msi-resume-title = 正在继续 { $app_title } 安装向导
msi-resume-description = 安装向导将在您的计算机上完成 { $app_title } 的安装。请单击“安装”继续，或单击“取消”退出安装向导。
msi-resume-btn-install = 安装(&I)

# MSI Installer UI - Shortcut Descriptions
msi-desktop-shortcut-description = { $app_title } 桌面快捷方式
msi-start-menu-shortcut-description = { $app_title } 开始菜单快捷方式
# MSI Installer UI - Readme Dialog
msi-readme-title = 自述信息
msi-readme-description = 请在继续之前阅读以下信息。

# MSI Installer UI - Downgrade Error
msi-downgrade-error = 已安装较新版本的 { $app_title }。

# Runtime download (install.rs, apply_windows_impl.rs, start_windows_impl.rs)
btn-retry = 重试
progress-cancelling = 正在取消…
runtime-prepare-header = 正在准备 { $app_title }
runtime-prepare-body = 正在检查运行环境…
runtime-download-starting = 正在下载运行环境…
runtime-downloading = 正在下载运行环境：{ $done } / { $total }
runtime-downloading-unknown = 正在下载运行环境：已下载 { $done }
runtime-verifying = 正在校验下载的文件…
runtime-unpacking = 正在解压运行环境…
runtime-finishing = 即将完成…
runtime-failed-header = 未能启动 { $app_title }
runtime-failed-download = 运行环境下载未完成。请检查网络连接后重试。
runtime-failed-invalid = 下载的文件未通过校验。请重试。
runtime-failed-other = 未能准备运行环境。请确认安装位置可以写入后重试。
setup-runtime-download = 运行环境下载未完成。请检查网络连接后重新运行安装程序。
setup-runtime-invalid = 下载的文件未通过校验。请重新运行安装程序。
setup-runtime-other = 未能准备运行环境。请确认安装位置可以写入后重新运行安装程序。

# Existing installation (install.rs)
btn-start = 启动
overwrite-incomplete-header = { $app_title } 需要修复
overwrite-incomplete-body = 这台电脑上的 { $app_title } 安装已损坏，需要修复安装。
