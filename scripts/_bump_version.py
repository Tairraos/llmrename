#!/usr/bin/env python3
"""bump patch 版本号：同步 tauri.conf.json / Cargo.toml / package.json。

用法: python3 scripts/_bump_version.py
输出: 新版本号（如 0.1.1），并写回三个文件。
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def bump(ver: str) -> str:
    parts = ver.split(".")
    while len(parts) < 3:
        parts.append("0")
    parts[2] = str(int(parts[2]) + 1)
    return ".".join(parts)

# 1) tauri.conf.json 是版本事实源
conf_path = ROOT / "src-tauri" / "tauri.conf.json"
conf = json.loads(conf_path.read_text())
old = conf["version"]
new = bump(old)
conf["version"] = new
conf_path.write_text(json.dumps(conf, indent=2, ensure_ascii=False) + "\n")

# 2) Cargo.toml [package] version
cargo_path = ROOT / "src-tauri" / "Cargo.toml"
text = cargo_path.read_text()
text = re.sub(
    r'(?m)^version = "[^"]+"',
    f'version = "{new}"',
    text,
    count=1,
)
cargo_path.write_text(text)

# 3) package.json version（一致性好维护）
pkg_path = ROOT / "package.json"
pkg = json.loads(pkg_path.read_text())
pkg["version"] = new
pkg_path.write_text(json.dumps(pkg, indent=2, ensure_ascii=False) + "\n")

print(new)