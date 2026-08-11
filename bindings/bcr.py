#!/usr/bin/env python3
"""bcr — Beyond Compare 风格文件对比工具的 Python 绑定(薄封装)。

设计(见 docs/P27-python-binding-design.md):
- 零第三方依赖(仅标准库 subprocess/json/dataclasses)
- 通过 subprocess 调用 bcr CLI,解析 `--json` 契约输出
- 返回类型化 dataclass;字段缺失容错;时间自动转 datetime
- 用法:
    import bcr
    r = bcr.compare("/a", "/b", content=True)
    if r.has_differences:
        bcr.sync("/a", "/b", mode="mirror")
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Optional

# bcr 可执行文件:环境变量 BCR_BIN 覆盖,默认 "bcr"(要求已加入 PATH)
BCR_BIN = os.environ.get("BCR_BIN", "bcr")


class Error(Exception):
    """bcr 调用失败(退出码 2 / 无输出 / JSON 解析失败)。"""


def _run(args: list[str]) -> dict:
    """执行 bcr 子命令并解析 JSON 契约输出。"""
    cmd = [BCR_BIN, *args, "--json"]
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")
    except FileNotFoundError:
        raise Error(f"找不到 bcr 可执行文件(设置 BCR_BIN 环境变量指向 bcr 二进制)") from None
    if not p.stdout.strip():
        raise Error(p.stderr.strip() or f"bcr 无输出,退出码 {p.returncode}")
    try:
        data = json.loads(p.stdout)
    except json.JSONDecodeError as e:
        raise Error(f"bcr JSON 解析失败: {e}\nstdout: {p.stdout[:500]}") from None
    if not data.get("ok", False):
        raise Error(data.get("error") or f"bcr 失败,退出码 {p.returncode}")
    data["_exit"] = p.returncode
    return data


def _parse_mtime(v: Any) -> Optional[datetime]:
    """ISO-8601 UTC → datetime;空值返回 None。"""
    if not v:
        return None
    try:
        return datetime.fromisoformat(v.replace("Z", "+00:00"))
    except ValueError:
        return None


@dataclass
class Meta:
    """单侧文件元数据。"""
    size: Optional[int] = None
    mtime: Optional[datetime] = None
    mode: Optional[int] = None
    symlink: Optional[str] = None

    @classmethod
    def from_json(cls, d: Any) -> "Meta | None":
        if not d:
            return None
        return cls(
            size=d.get("size"),
            mtime=_parse_mtime(d.get("mtime")),
            mode=d.get("mode"),
            symlink=d.get("symlink"),
        )


@dataclass
class Entry:
    """单条比较结果。status: same|left_only|right_only|differ|moved"""
    rel: str
    status: str
    left: Optional[Meta] = None
    right: Optional[Meta] = None
    moved_to: Optional[str] = None
    attrs_differ: bool = False


@dataclass
class Stats:
    same: int = 0
    left_only: int = 0
    right_only: int = 0
    differ: int = 0
    moved: int = 0


@dataclass
class CompareResult:
    stats: Stats
    has_differences: bool
    entries: list[Entry]
    warnings: list[str] = field(default_factory=list)
    raw: dict = field(default_factory=dict)

    @property
    def differences(self) -> list[Entry]:
        """便捷过滤:仅差异条目(排除 same)。"""
        return [e for e in self.entries if e.status != "same"]


@dataclass
class SyncPlanItem:
    op: str          # copy|delete|rename|rmdir|skip|conflict
    rel: str = ""
    to: Optional[str] = None
    from_: Optional[str] = None
    reason: Optional[str] = None
    size: Optional[int] = None


@dataclass
class SyncStats:
    copy: int = 0
    delete: int = 0
    rename: int = 0
    rmdir: int = 0
    skip: int = 0
    conflict: int = 0
    errors: int = 0


@dataclass
class SyncResult:
    dry_run: bool
    mode: str
    plan: list[SyncPlanItem]
    stats: SyncStats
    raw: dict = field(default_factory=dict)


def compare(
    left: str,
    right: str,
    *,
    content: bool = False,
    includes: Optional[list[str]] = None,
    excludes: Optional[list[str]] = None,
    show_same: bool = False,
    detect_moves: bool = True,
    compare_attrs: bool = False,
    compare_version: bool = False,
    profile: Optional[str] = None,
) -> CompareResult:
    """目录/文件对比,返回结构化结果。"""
    args = ["compare", left, right]
    if content:
        args.append("--compare-content")
    for g in includes or []:
        args += ["--include", g]
    for g in excludes or []:
        args += ["--exclude", g]
    if show_same:
        args.append("--show-same")
    if not detect_moves:
        args.append("--detect-moves=false")
    if compare_attrs:
        args.append("--compare-attrs")
    if compare_version:
        args.append("--compare-version")
    if profile:
        args += ["--profile", profile]
    data = _run(args)
    r = data["result"]
    s = r.get("stats", {})
    return CompareResult(
        stats=Stats(**{k: s.get(k, 0) for k in ("same", "left_only", "right_only", "differ", "moved")}),
        has_differences=r.get("has_differences", False),
        entries=[
            Entry(
                rel=e["rel"],
                status=e["status"],
                left=Meta.from_json(e.get("left")),
                right=Meta.from_json(e.get("right")),
                moved_to=e.get("moved_to"),
                attrs_differ=e.get("attrs_differ", False),
            )
            for e in r.get("entries", [])
        ],
        warnings=data.get("warnings", []),
        raw=r,
    )


def sync(
    left: str,
    right: str,
    *,
    mode: str = "update",
    reverse: bool = False,
    dry_run: bool = False,
    content: bool = False,
    includes: Optional[list[str]] = None,
    excludes: Optional[list[str]] = None,
) -> SyncResult:
    """目录同步;dry_run=True 只返回计划不执行。"""
    args = ["sync", left, right, "--mode", mode]
    if reverse:
        args.append("--reverse")
    if dry_run:
        args.append("--dry-run")
    if content:
        args.append("--compare-content")
    for g in includes or []:
        args += ["--include", g]
    for g in excludes or []:
        args += ["--exclude", g]
    data = _run(args)
    r = data["result"]
    plan = [
        SyncPlanItem(
            op=p.get("op", ""),
            rel=p.get("rel", ""),
            to=p.get("to"),
            from_=p.get("from"),
            reason=p.get("reason"),
            size=p.get("size"),
        )
        for p in r.get("plan", [])
    ]
    st = r.get("stats", {})
    return SyncResult(
        dry_run=r.get("dry_run", False),
        mode=r.get("mode", mode),
        plan=plan,
        stats=SyncStats(**{k: st.get(k, 0) for k in ("copy", "delete", "rename", "rmdir", "skip", "conflict", "errors")}),
        raw=r,
    )


def run(args: list[str]) -> dict:
    """底层透传:执行任意 bcr 子命令并返回解析后的 JSON dict。"""
    return _run(args)


if __name__ == "__main__":
    # 简易自检:python3 bcr.py /a /b
    if len(sys.argv) == 3:
        r = compare(sys.argv[1], sys.argv[2])
        print(f"same={r.stats.same} left_only={r.stats.left_only} "
              f"right_only={r.stats.right_only} differ={r.stats.differ} moved={r.stats.moved}")
        for e in r.differences:
            print(f"  [{e.status}] {e.rel}")
        sys.exit(1 if r.has_differences else 0)
    print(__doc__)
