#!/usr/bin/env swift
// P39 UI 学习：截图 BC 当前窗口（按 owner 名匹配）
// 用法: swift bc_shot.swift <输出png路径>
import Cocoa
import CoreGraphics

let outPath = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "/tmp/bc_shot.png"

// 用 CGWindowList 找 Beyond Compare 窗口
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let list = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else {
    print("无法获取窗口列表"); exit(1)
}
var targetID: CGWindowID?
for w in list {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    if owner.contains("Beyond Compare") || owner.contains("BCompare") {
        targetID = w[kCGWindowNumber as String] as? CGWindowID
        let name = w[kCGWindowName as String] as? String ?? ""
        print("找到窗口: \(owner) | \(name) | id=\(targetID ?? 0)")
        break
    }
}
guard let wid = targetID else {
    print("未找到 Beyond Compare 窗口"); exit(1)
}

// 截取指定窗口
guard let img = CGWindowListCreateImage(.null, .optionIncludingWindow, wid, [.boundsIgnoreFraming]) else {
    print("截图失败"); exit(1)
}
let rep = NSBitmapImageRep(cgImage: img)
guard let data = rep.representation(using: .png, properties: [:]) else {
    print("PNG 编码失败"); exit(1)
}
try? data.write(to: URL(fileURLWithPath: outPath))
print("已保存 \(outPath)")
