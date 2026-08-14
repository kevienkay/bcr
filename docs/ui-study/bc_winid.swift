#!/usr/bin/env swift
// P39 UI 学习：打印 Beyond Compare 窗口 id（供 screencapture -l 使用）
import Cocoa
import CoreGraphics

let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let list = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else {
    print("无法获取窗口列表"); exit(1)
}
for w in list {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    if owner.contains("Beyond Compare") || owner.contains("BCompare") {
        let id = w[kCGWindowNumber as String] as? Int ?? 0
        let name = w[kCGWindowName as String] as? String ?? ""
        let bounds = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
        print("\(id) | \(owner) | \(name) | \(bounds)")
    }
}
