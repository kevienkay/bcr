#!/usr/bin/env swift
// P39 UI 学习：在文件对话框中输入路径并打开（补丁视图等）
// 用法: swift bc_open_file.swift <路径>
import Cocoa
import ApplicationServices

let path = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "/tmp/bc-study/change.patch"

func axValue(_ el: AXUIElement, _ attr: String) -> CFTypeRef? {
    var v: CFTypeRef?
    let e = AXUIElementCopyAttributeValue(el, attr as CFString, &v)
    return e == .success ? v : nil
}
func axChildren(_ el: AXUIElement) -> [AXUIElement] { axValue(el, kAXChildrenAttribute) as? [AXUIElement] ?? [] }

// 递归找 text field 并设置值
func findAndSet(_ el: AXUIElement, _ depth: Int) -> Bool {
    if depth > 8 { return false }
    let role = axValue(el, kAXRoleAttribute) as? String ?? ""
    if role == kAXTextFieldRole as String {
        let err = AXUIElementSetAttributeValue(el, kAXValueAttribute as CFString, path as CFTypeRef)
        if err == .success {
            print("已设置 text field 值: \(path)")
            return true
        }
    }
    for c in axChildren(el) {
        if findAndSet(c, depth + 1) { return true }
    }
    return false
}

let ws = NSWorkspace.shared
let pid = ws.runningApplications.filter { $0.localizedName?.contains("Beyond Compare") == true }.first?.processIdentifier ?? 0
let app = AXUIElementCreateApplication(pid)

// 遍历窗口 + sheet
if let wins = axValue(app, kAXWindowsAttribute) as? [AXUIElement] {
    for w in wins {
        if findAndSet(w, 0) { break }
        if let sheets = axValue(w, "AXSheets") as? [AXUIElement] {
            for s in sheets {
                if findAndSet(s, 0) { break }
            }
        }
    }
}
