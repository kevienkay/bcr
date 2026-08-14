#!/usr/bin/env swift
// P39 UI 学习：读 BC 当前窗口的菜单栏 + 工具栏按钮结构（AX API）
// 用法: swift bc_ui_scan.swift [输出文件]
import Cocoa
import ApplicationServices

let outPath = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "/tmp/bc_ui_scan.txt"
var out = ""

func axValue(_ el: AXUIElement, _ attr: String) -> CFTypeRef? {
    var value: CFTypeRef?
    let err = AXUIElementCopyAttributeValue(el, attr as CFString, &value)
    return err == .success ? value : nil
}

func axStr(_ el: AXUIElement, _ attr: String) -> String {
    axValue(el, attr) as? String ?? ""
}

func axEl(_ el: AXUIElement, _ attr: String) -> AXUIElement? {
    axValue(el, attr) as! AXUIElement?
}

func axChildren(_ el: AXUIElement) -> [AXUIElement] {
    axValue(el, kAXChildrenAttribute) as? [AXUIElement] ?? []
}

// 递归打印菜单项：名称 + 快捷键 + 层级
func dumpMenu(_ menu: AXUIElement, _ depth: Int) {
    let indent = String(repeating: "  ", count: depth)
    for item in axChildren(menu) {
        let title = axStr(item, kAXTitleAttribute)
        let cmd = axStr(item, kAXMenuItemCmdCharAttribute)
        let mods = (axValue(item, kAXMenuItemCmdModifiersAttribute) as? NSNumber)?.uintValue ?? 0
        var modStr = ""
        if mods & (1 << 0) != 0 { modStr += "⌘" }
        if mods & (1 << 1) != 0 { modStr += "⇧" }
        if mods & (1 << 2) != 0 { modStr += "⌥" }
        if mods & (1 << 3) != 0 { modStr += "⌃" }
        let enabled = axValue(item, kAXEnabledAttribute) as? Bool ?? true
        let mark = enabled ? "" : " [disabled]"
        let shortcut = cmd.isEmpty ? "" : "  (\(modStr)\(cmd))"
        out += "\(indent)\(title)\(shortcut)\(mark)\n"
        // 子菜单（MenuItem 的 children 里包含子菜单）
        for c in axChildren(item) {
            if let role = axValue(c, kAXRoleAttribute) as? String, role == kAXMenuRole as String {
                dumpMenu(c, depth + 1)
            }
        }
    }
}

// 通过 NSWorkspace 找 Beyond Compare 进程
let ws = NSWorkspace.shared
let pids = ws.runningApplications.filter { $0.localizedName?.contains("Beyond Compare") == true }.map { $0.processIdentifier }
guard let pid = pids.first else {
    print("未找到 Beyond Compare 进程"); exit(1)
}
let app = AXUIElementCreateApplication(pid)

// 菜单栏
if let menubar = axEl(app, kAXMenuBarAttribute) {
    out += "=== 菜单栏 ===\n"
    dumpMenu(menubar, 0)
}
// 窗口 + 工具栏
if let windows = axValue(app, kAXWindowsAttribute) as? [AXUIElement] {
    for (wi, w) in windows.enumerated() {
        let title = axStr(w, kAXTitleAttribute)
        out += "\n=== 窗口 \(wi): \(title) ===\n"
        // 工具栏按钮
        if let toolbar = axEl(w, "AXToolbar") {
            out += "--- 工具栏 ---\n"
            for btn in axChildren(toolbar) {
                let role = axStr(btn, kAXRoleAttribute)
                let desc = axStr(btn, kAXDescriptionAttribute)
                let title = axStr(btn, kAXTitleAttribute)
                let help = axStr(btn, kAXHelpAttribute)
                out += "  [\(role)] title=\(title) desc=\(desc) help=\(help)\n"
            }
        }
        // 窗口直属按钮/输入框
        for btn in axChildren(w) {
            let role = axStr(btn, kAXRoleAttribute)
            if role == kAXButtonRole as String || role == kAXTextFieldRole as String {
                let desc = axStr(btn, kAXDescriptionAttribute)
                let title = axStr(btn, kAXTitleAttribute)
                out += "  w[\(role)] title=\(title) desc=\(desc)\n"
            }
        }
    }
}
try? out.write(toFile: outPath, atomically: true, encoding: .utf8)
print("已写入 \(outPath) (\(out.count) 字符)")
