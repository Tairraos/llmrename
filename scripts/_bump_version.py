#!/usr/bin/env python3
"""bump 版本号：同步 tauri.conf.json / Cargo.toml / package.json。

用法:
  python3 scripts/_bump_version.py        # patch 版本 +1
  python3 scripts/_bump_version.py 1.0.0  # 显式设置版本号（大版本发布用）
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
explicit = sys.argv[1] if len(sys.argv) > 1 else None
if explicit is not None:
    if not re.fullmatch(r"\d+\.\d+\.\d+", explicit):
        sys.exit(f"版本号格式非法：{explicit}（应为 X.Y.Z）")
    new = explicit
else:
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