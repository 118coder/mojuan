#!/usr/bin/env python3
"""把本地仓库发布到 GitHub（走 REST API，绕开被墙的 git 协议）。

用法：
    set GITHUB_TOKEN=ghp_xxx
    python tools/publish.py <owner/repo> [--private]

做的事：
  1. 仓库不存在就创建（已存在则复用）
  2. 用 Git Data API 上传当前 HEAD 的所有文件（blobs → tree → commit → ref）
  3. 打印仓库地址和 Actions 页面

为什么不用 git push：部分网络环境只放行 api.github.com，github.com:443 的
git 协议会失败。API 上传同样能产生真实提交，只是历史被压成一个 commit。
"""

import base64
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request

API = "https://api.github.com"


def token() -> str:
    t = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if not t:
        sys.exit("缺少 GITHUB_TOKEN 环境变量（需要 repo + workflow 权限）")
    return t.strip()


def call(method: str, path: str, tok: str, payload=None):
    url = API + path
    data = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"Bearer {tok}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("User-Agent", "mojuan-publisher")
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            body = r.read()
            return json.loads(body) if body else {}
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")
        raise SystemExit(f"{method} {path} -> {e.code}\n{detail}")


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], capture_output=True, text=True, check=True
    ).stdout.strip()


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit("用法：python tools/publish.py <owner/repo> [--private]")
    full = sys.argv[1]
    private = "--private" in sys.argv
    if full.count("/") != 1:
        sys.exit("仓库名格式应为 owner/repo")
    owner, name = full.split("/")
    tok = token()

    me = call("GET", "/user", tok)
    if me.get("login", "").lower() != owner.lower():
        print(f"注意：token 属于 {me.get('login')}，目标 owner 是 {owner}")

    # 1) 仓库
    try:
        repo = call("GET", f"/repos/{full}", tok)
        print(f"仓库已存在：{repo['html_url']}")
    except SystemExit as e:
        if "404" not in str(e):
            raise
        repo = call(
            "POST",
            "/user/repos",
            tok,
            {
                "name": name,
                "description": "墨卷 · 纯 Rust 的 Markdown 阅读与编辑器（单文件 exe）",
                "private": private,
                "has_issues": True,
                "has_wiki": False,
                "auto_init": False,
            },
        )
        print(f"已创建仓库：{repo['html_url']}")

    branch = "main"

    # 2) 上传文件
    files = [f for f in git("ls-files").splitlines() if f.strip()]
    if not files:
        sys.exit("git ls-files 为空，先 git add/commit")
    print(f"准备上传 {len(files)} 个文件…")

    tree = []
    for i, path in enumerate(files, 1):
        with open(path, "rb") as fh:
            raw = fh.read()
        blob = call(
            "POST",
            f"/repos/{full}/git/blobs",
            tok,
            {"content": base64.b64encode(raw).decode(), "encoding": "base64"},
        )
        tree.append({"path": path, "mode": "100644", "type": "blob", "sha": blob["sha"]})
        if i % 10 == 0 or i == len(files):
            print(f"  {i}/{len(files)}")

    new_tree = call("POST", f"/repos/{full}/git/trees", tok, {"tree": tree})
    message = git("log", "-1", "--pretty=%B").strip() or "chore: initial import"
    commit = call(
        "POST",
        f"/repos/{full}/git/commits",
        tok,
        {"message": message, "tree": new_tree["sha"]},
    )

    # 3) 更新或创建分支
    try:
        call("PATCH", f"/repos/{full}/git/refs/heads/{branch}", tok, {"sha": commit["sha"], "force": True})
        print(f"已更新分支 {branch}")
    except SystemExit:
        call(
            "POST",
            f"/repos/{full}/git/refs",
            tok,
            {"ref": f"refs/heads/{branch}", "sha": commit["sha"]},
        )
        print(f"已创建分支 {branch}")

    call("PATCH", f"/repos/{full}", tok, {"default_branch": branch})

    print("\n完成：")
    print(f"  仓库    {repo['html_url']}")
    print(f"  提交    {commit['sha'][:10]}")
    print(f"  Actions {repo['html_url']}/actions")
    print("\n下一步：打 tag 触发自动发布")
    print(f"  git tag v0.1.0 && git push origin v0.1.0")
    print("（如果 git push 不通，可在网页上创建 Release 并上传 dist/mojuan.exe）")


if __name__ == "__main__":
    main()
