#!/usr/bin/env swift
// P39 UI 学习：递归打印某窗口/元素的 AX 树（调试对话框用）
// 用法: swift bc_ax_dump.swift <windowName子串>
import Cocoa
import ApplicationServices

let filter = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "Open"
var out = ""

func axValue(_ el: AXUIElement, _ attr: String) -> CFTypeRef? {
    var value: CFTypeRef?
    let err = AXUIElementCopyAttributeValue(el, attr as CFString, &value)
    return err == .success ? value : nil
}
func axStr(_ el: AXUIElement, _ attr: String) -> String {
    axValue(el, attr) as? String ?? ""
}
func axChildren(_ el: AXUIElement) -> [AXUIElement] {
    axValue(el, kAXChildrenAttribute) as? [AXUIElement] ?? []
}

func dump(_ el: AXUIElement, _ depth: Int) {
    let indent = String(repeating: "  ", count: depth)
    let role = axStr(el, kAXRoleAttribute)
    let title = axStr(el, kAXTitleAttribute)
    let desc = axStr(el, kAXDescriptionAttribute)
    let val = axStr(el, kAXValueAttribute)
    if depth < 6 {
        out += "\(indent)[\(role)] title=\(title) desc=\(desc) val=\(val)\n"
    }
    if depth < 5 {
        for c in axChildren(el) { dump(c, depth + 1) }
    }
}

let ws = NSWorkspace.shared
let pid = ws.runningApplications.filter { $0.localizedName?.contains("Beyond Compare") == true }.first?.processIdentifier ?? 0
let app = AXUIElementCreateApplication(pid)
if let windows = axValue(app, kAXWindowsAttribute) as? [AXUIElement] {
    for w in windows {
        let t = axStr(w, kAXTitleAttribute)
        if t.lowercased().contains(filter.lowercased()) || filter == "ALL" {
            out += "=== 窗口: \(t) ===\n"
            dump(w, 0)
        }
    }
}
print(out)
