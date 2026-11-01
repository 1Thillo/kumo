"""Writes assets/third-party-notices.txt: who wrote the crates inside kumo.exe and under which
license. Their licenses ask for this text to ship with the program, and the panel opens it.
Run it after changing dependencies:  python tools/third_party.py
With --check it only tells whether the file is current (the automated build does this).
"""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "third-party-notices.txt"
TARGET = "x86_64-pc-windows-msvc"

# Where a crate offers a choice, the first of these it offers is the one Kumo uses.
PREFERRED = ["MIT", "BSD-3-Clause", "BSD-2-Clause", "Zlib", "Apache-2.0", "BSL-1.0", "Unlicense", "CC0-1.0", "0BSD"]

MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy of this software
and associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE."""

HEADER = """Third-party notices for Kumo

Kumo is free software under the GNU General Public License v3. It is built with the libraries
listed here. Thank you to everyone who wrote them. Their licenses come after the list.

"""


def cargo(*args):
    return subprocess.check_output(["cargo", *args, "--locked"], cwd=ROOT, encoding="utf-8")


def crates():
    """Every crate that goes into the exe, with its cargo metadata."""
    meta = json.loads(cargo("metadata", "--format-version", "1", "--filter-platform", TARGET))
    known = {(p["name"], p["version"]): p for p in meta["packages"]}
    used = set()
    for line in cargo("tree", "-e", "normal", "--prefix", "none", "--format", "{p}", "--target", TARGET).splitlines():
        m = re.match(r"(\S+) v(\S+)", line)
        if m and m[1] != "kumo":
            used.add((m[1], m[2]))
    return [known[k] for k in sorted(used)]


def chosen(expression):
    """The licenses Kumo takes up from a crate's license expression."""
    expression = expression.replace("/", " OR ").replace("(", "").replace(")", "")
    if "GPL-3.0-only" in expression:
        return ["GPL-3.0-only"]
    parts = []
    for term in expression.split(" AND "):
        options = term.split(" OR ")
        parts.append(next((p for p in PREFERRED if p in options), options[0]).strip())
    return parts


def license_files(folder):
    """The license files of an unpacked crate, with what their names say about them."""
    found = []
    for path in sorted(folder.iterdir()):
        if path.is_dir() and path.name.lower() in ("licenses", "license"):
            found += [p for p in sorted(path.iterdir()) if p.is_file()]
        elif path.is_file() and re.match(r"(?i)(licen[cs]e|copying|unlicense|notice|copyright)", path.name):
            found.append(path)
    return found


def matches(path, license):
    name = path.stem.lower() + path.suffix.lower()
    key = {"Apache-2.0": "apache", "BSD-3-Clause": "bsd", "BSD-2-Clause": "bsd", "CC0-1.0": "cc0", "BSL-1.0": "boost"}.get(license, license.lower())
    return key in name


def own_texts(crate, licenses):
    """What a crate itself ships for the licenses used: named files, else its plain LICENSE."""
    files = license_files(pathlib.Path(crate["manifest_path"]).parent)
    known = ("mit", "apache", "bsd", "zlib", "unicode", "cc0", "boost", "unlicense", "0bsd")
    keep = [f for f in files if any(matches(f, l) for l in licenses)]
    if not keep:
        keep = [f for f in files if not any(k in f.name.lower() for k in known)]
    keep += [f for f in files if f.name.lower().startswith("notice") and f not in keep]
    return [f.read_text(encoding="utf-8", errors="replace").strip() for f in keep]


def build():
    list_lines, groups, by_license, missing = [], {}, {}, []
    all_crates = crates()
    for c in all_crates:
        licenses = chosen(c["license"] or "")
        where = c["repository"] or c["homepage"] or ""
        authors = ", ".join(a.split(" <")[0] for a in c["authors"])
        list_lines.append(f'{c["name"]} {c["version"]}  |  {" AND ".join(licenses)}  |  {authors}  |  {where}'.rstrip(" |"))
        if licenses == ["GPL-3.0-only"]:
            continue
        texts = own_texts(c, licenses)
        if not texts:
            missing.append((c, licenses))
        for text in texts:
            groups.setdefault(text, []).append(f'{c["name"]} {c["version"]}')
            if len(licenses) == 1:
                by_license.setdefault(licenses[0], text)
    by_license.setdefault("MIT", MIT_TEXT)
    for c, licenses in missing:
        authors = ", ".join(a.split(" <")[0] for a in c["authors"]) or f'the {c["name"]} authors'
        for license in licenses:
            text = f"Copyright (c) {authors}\n\n{MIT_TEXT}" if license == "MIT" else by_license.get(license)
            if text is None:
                sys.exit(f'no license text for {c["name"]} ({license})')
            groups.setdefault(text, []).append(f'{c["name"]} {c["version"]}')

    out = [HEADER, f"LIBRARIES ({len(all_crates)})\n\n", "\n".join(list_lines), "\n\n"]
    out.append("Slint is used under the GNU GPL v3, the same license as Kumo. See About Kumo, License.\n\n")
    out.append("LICENSES\n")
    for text, names in sorted(groups.items(), key=lambda g: g[1][0]):
        out += ["\n", "=" * 78, "\n", "Applies to: ", ", ".join(names), "\n\n", text, "\n"]
    return "".join(out).replace("\r\n", "\n")


text = build()
if "--check" in sys.argv:
    if not OUT.exists() or OUT.read_text(encoding="utf-8").replace("\r\n", "\n") != text:
        sys.exit("assets/third-party-notices.txt is out of date: run python tools/third_party.py")
    print("third-party notices are current")
else:
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(f"{text.count(chr(10))} lines, {len(text.encode()) // 1024} KB")
