#!/usr/bin/env swift
// P39 UI 学习：dump 文件对话框 sheet 完整 AX 结构（含按钮/文本字段）
import Cocoa
import ApplicationServices

func axValue(_ el: AXUIElement, _ attr: String) -> CFTypeRef? {
    var v: CFTypeRef?
    let e = AXUIElementCopyAttributeValue(el, attr as CFString, &v)
    return e == .success ? v : nil
}
func axStr(_ el: AXUIElement, _ attr: String) -> String { axValue(el, attr) as? String ?? "" }
func axChildren(_ el: AXUIElement) -> [AXUIElement] { axValue(el, kAXChildrenAttribute) as? [AXUIElement] ?? [] }

func dump(_ el: AXUIElement, _ d: Int) {
    let role = axStr(el, kAXRoleAttribute)
    let title = axStr(el, kAXTitleAttribute)
    let desc = axStr(el, kAXDescriptionAttribute)
    let val = axStr(el, kAXValueAttribute)
    let sub = axStr(el, kAXSubroleAttribute)
    print(String(repeating: "  ", count: d) + "[\(role)/\(sub)] t=\(title) d=\(desc) v=\(val)")
    if d < 7 { for c in axChildren(el) { dump(c, d + 1) } }
}

let ws = NSWorkspace.shared
let pid = ws.runningApplications.filter { $0.localizedName?.contains("Beyond Compare") == true }.first?.processIdentifier ?? 0
let app = AXUIElementCreateApplication(pid)
if let wins = axValue(app, kAXWindowsAttribute) as? [AXUIElement] {
    for w in wins {
        if let sheets = axValue(w, "AXSheets") as? [AXUIElement] {
            for s in sheets {
                print("=== SHEET on \(axStr(w, kAXTitleAttribute)) ===")
                dump(s, 0)
            }
        }
    }
}
