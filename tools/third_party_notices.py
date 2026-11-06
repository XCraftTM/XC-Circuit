"""Writes THIRD-PARTY-NOTICES.txt: every Rust crate built into the app, with
its licence and the licence texts it ships.

Usage: python tools/third_party_notices.py [out-file] [target]   (from the repository root)
The target (such as x86_64-pc-windows-msvc) defaults to this computer's.
"""
import hashlib
import json
import os
import subprocess
import sys

# The crate that ends up in a release: the app.
ROOTS = ["circuit-editor"]
LICENCE_FILES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "THIRD-PARTY-NOTICES.txt"
    target = sys.argv[2] if len(sys.argv) > 2 else host_target()
    # Only what is built for this platform.
    meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target]))
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    ours = {p["id"] for p in meta["packages"] if p["source"] is None}

    # What the roots need to build and run (not their tests).
    todo = [i for i, p in packages.items() if p["name"] in ROOTS]
    seen = set()
    while todo:
        i = todo.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]["deps"]:
            if any(k["kind"] in (None, "normal") for k in dep["dep_kinds"]):
                todo.append(dep["pkg"])

    crates = sorted((packages[i] for i in seen if i not in ours), key=lambda p: (p["name"], p["version"]))
    lines = [
        "XC(TM) Circuit includes the following third-party software.",
        "Each is used under the licence named with it; the licence texts follow.",
        "",
    ]
    for p in crates:
        lines.append(f"- {p['name']} {p['version']}: {p.get('license') or 'see its licence file'}")
    lines.append("")
    shown = {}
    for p in crates:
        folder = os.path.dirname(p["manifest_path"])
        lines += ["=" * 78, f"{p['name']} {p['version']}", f"Licence: {p.get('license') or 'see below'}"]
        if p.get("repository"):
            lines.append(f"Source: {p['repository']}")
        lines.append("=" * 78)
        texts = sorted(f for f in os.listdir(folder) if f.upper().startswith(LICENCE_FILES) and os.path.isfile(os.path.join(folder, f)))
        for f in texts:
            with open(os.path.join(folder, f), encoding="utf-8", errors="replace") as fh:
                text = fh.read().strip()
            # Each text once: many crates ship the very same Apache or MIT text.
            key = hashlib.sha256(" ".join(text.split()).encode()).hexdigest()
            if key in shown:
                lines += ["", f"--- {f}: the same text as {shown[key]} above ---"]
            else:
                shown[key] = f"{f} of {p['name']} {p['version']}"
                lines += ["", f"--- {f} ---", text]
        if not texts:
            lines += ["", f"(No licence file in the package. Licence: {p.get('license') or 'unknown'}.)"]
        lines.append("")
    with open(out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(lines) + "\n")
    print(f"{out}: {len(crates)} crates")


def host_target():
    info = subprocess.check_output(["rustc", "-vV"], text=True)
    return next(l.split(":", 1)[1].strip() for l in info.splitlines() if l.startswith("host:"))


if __name__ == "__main__":
    main()
