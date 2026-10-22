"""Genera legal/THIRD_PARTY_LICENSES.<idioma>.txt con los avisos de licencia de
las librerías que van dentro de la app (Rust para Windows + paquetes npm del
frontend), en los 6 idiomas de la app.

Los textos de licencia en sí se copian tal cual (en su idioma original: es lo
que exige cada licencia); solo se traducen los encabezados y las etiquetas. Los
textos idénticos se agrupan para que el archivo no se infle: cada texto sale una
vez con la lista de paquetes que lo usan.

Uso (desde la raíz del repo):  python scripts/generate-third-party-licenses.py
"""

import hashlib
import json
import os
import subprocess
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEGAL = os.path.join(ROOT, "legal")
TARGET = "x86_64-pc-windows-msvc"
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")

STRINGS = {
    "es": {
        "title": "PROJECT CIEL — LICENCIAS DE TERCEROS",
        "intro": [
            "Project Ciel incluye las siguientes librerías de código abierto. Cada texto de licencia aparece",
            "una vez (en su idioma original, como exige cada licencia), seguido de la lista de paquetes a los que se aplica.",
        ],
        "packages": "Paquetes",
        "authors": "autores",
        "source": "código fuente",
        "unknown": "sin licencia declarada",
        "missing": [
            "Paquetes sin archivo de licencia en su carpeta (se indica la licencia declarada en su manifiesto).",
            "El texto estándar de cada licencia aparece más arriba.",
        ],
        "models": [
            "Modelos que la app descarga cuando el usuario lo pide (no se incluyen en el instalador):",
            "  Whisper (OpenAI) y whisper.cpp (ggml-org) — licencia MIT",
            "  paraphrase-multilingual-MiniLM-L12-v2 (sentence-transformers) — licencia Apache-2.0",
            "  Obscura (h4ckf0r0day, navegador sin ventana para leer páginas web) — licencia Apache-2.0",
        ],
    },
    "en": {
        "title": "PROJECT CIEL — THIRD-PARTY LICENSES",
        "intro": [
            "Project Ciel includes the following open-source libraries. Each license text appears once",
            "(in its original language, as each license requires), followed by the list of packages it applies to.",
        ],
        "packages": "Packages",
        "authors": "authors",
        "source": "source code",
        "unknown": "no declared license",
        "missing": [
            "Packages without a license file in their folder (the license declared in their manifest is shown).",
            "The standard text of each license appears above.",
        ],
        "models": [
            "Models the app downloads when the user asks for them (not included in the installer):",
            "  Whisper (OpenAI) and whisper.cpp (ggml-org) — MIT license",
            "  paraphrase-multilingual-MiniLM-L12-v2 (sentence-transformers) — Apache-2.0 license",
            "  Obscura (h4ckf0r0day, headless browser used to read web pages) — Apache-2.0 license",
        ],
    },
    "pt": {
        "title": "PROJECT CIEL — LICENÇAS DE TERCEIROS",
        "intro": [
            "O Project Ciel inclui as seguintes bibliotecas de código aberto. Cada texto de licença aparece",
            "uma vez (no idioma original, como cada licença exige), seguido da lista de pacotes aos quais se aplica.",
        ],
        "packages": "Pacotes",
        "authors": "autores",
        "source": "código-fonte",
        "unknown": "sem licença declarada",
        "missing": [
            "Pacotes sem arquivo de licença na sua pasta (indica-se a licença declarada no manifesto).",
            "O texto padrão de cada licença aparece acima.",
        ],
        "models": [
            "Modelos que o app baixa quando o usuário pede (não estão incluídos no instalador):",
            "  Whisper (OpenAI) e whisper.cpp (ggml-org) — licença MIT",
            "  paraphrase-multilingual-MiniLM-L12-v2 (sentence-transformers) — licença Apache-2.0",
            "  Obscura (h4ckf0r0day, navegador sem janela para ler páginas web) — licença Apache-2.0",
        ],
    },
    "ja": {
        "title": "PROJECT CIEL — サードパーティライセンス",
        "intro": [
            "Project Ciel には以下のオープンソースライブラリが含まれています。各ライセンス本文は",
            "（各ライセンスの定めにより原文のまま）1回だけ掲載し、その後に適用されるパッケージの一覧を示します。",
        ],
        "packages": "パッケージ数",
        "authors": "作者",
        "source": "ソースコード",
        "unknown": "ライセンス記載なし",
        "missing": [
            "フォルダ内にライセンスファイルがないパッケージ（マニフェストに記載されたライセンスを示します）。",
            "各ライセンスの標準的な本文は上に掲載しています。",
        ],
        "models": [
            "ユーザーの操作でアプリがダウンロードするモデル（インストーラーには含まれません）:",
            "  Whisper（OpenAI）および whisper.cpp（ggml-org）— MIT ライセンス",
            "  paraphrase-multilingual-MiniLM-L12-v2（sentence-transformers）— Apache-2.0 ライセンス",
            "  Obscura（h4ckf0r0day、Webページを読むためのヘッドレスブラウザ）— Apache-2.0 ライセンス",
        ],
    },
    "ko": {
        "title": "PROJECT CIEL — 제3자 라이선스",
        "intro": [
            "Project Ciel에는 다음 오픈소스 라이브러리가 포함되어 있습니다. 각 라이선스 본문은",
            "(각 라이선스의 요구에 따라 원문 그대로) 한 번만 실었으며, 이어서 적용되는 패키지 목록을 표시합니다.",
        ],
        "packages": "패키지 수",
        "authors": "저자",
        "source": "소스 코드",
        "unknown": "라이선스 명시 없음",
        "missing": [
            "폴더에 라이선스 파일이 없는 패키지(매니페스트에 명시된 라이선스를 표시합니다).",
            "각 라이선스의 표준 본문은 위에 있습니다.",
        ],
        "models": [
            "사용자가 요청할 때 앱이 다운로드하는 모델(설치 프로그램에는 포함되지 않음):",
            "  Whisper(OpenAI) 및 whisper.cpp(ggml-org) — MIT 라이선스",
            "  paraphrase-multilingual-MiniLM-L12-v2(sentence-transformers) — Apache-2.0 라이선스",
            "  Obscura(h4ckf0r0day, 웹 페이지를 읽기 위한 헤드리스 브라우저) — Apache-2.0 라이선스",
        ],
    },
    "zh": {
        "title": "PROJECT CIEL — 第三方许可",
        "intro": [
            "Project Ciel 包含以下开源库。每份许可文本只出现一次（按各许可的要求保留原文），",
            "随后列出适用该文本的软件包。",
        ],
        "packages": "软件包数量",
        "authors": "作者",
        "source": "源代码",
        "unknown": "未声明许可",
        "missing": [
            "文件夹中没有许可文件的软件包（显示其清单中声明的许可）。",
            "各许可的标准文本见上文。",
        ],
        "models": [
            "应用在用户要求时下载的模型（不包含在安装程序中）：",
            "  Whisper（OpenAI）和 whisper.cpp（ggml-org）— MIT 许可",
            "  paraphrase-multilingual-MiniLM-L12-v2（sentence-transformers）— Apache-2.0 许可",
            "  Obscura（h4ckf0r0day，用于读取网页的无头浏览器）— Apache-2.0 许可",
        ],
    },
}


def run(cmd, cwd):
    return subprocess.run(cmd, cwd=cwd, check=True, capture_output=True, text=True, encoding="utf-8").stdout


def license_files(directory):
    found = []
    try:
        names = sorted(os.listdir(directory))
    except OSError:
        return found
    for name in names:
        if name.upper().startswith(LICENSE_PREFIXES) and os.path.isfile(os.path.join(directory, name)):
            with open(os.path.join(directory, name), encoding="utf-8", errors="replace") as handle:
                text = handle.read().strip()
            if text:
                found.append(text)
    return found


def rust_packages():
    src_tauri = os.path.join(ROOT, "src-tauri")
    tree = run(
        ["cargo", "tree", "-e", "normal,build", "--target", TARGET, "--prefix", "none", "--format", "{p}"],
        src_tauri,
    )
    wanted = set()
    for line in tree.splitlines():
        parts = line.strip().split(" ")
        if len(parts) >= 2 and parts[1].startswith("v") and "(" not in line:
            wanted.add((parts[0], parts[1][1:]))
    metadata = json.loads(run(["cargo", "metadata", "--format-version", "1", "--locked"], src_tauri))
    workspace = set(metadata["workspace_members"])
    packages = []
    for pkg in metadata["packages"]:
        if pkg["id"] in workspace or (pkg["name"], pkg["version"]) not in wanted:
            continue
        authors = ", ".join(pkg["authors"]) if pkg.get("authors") else ""
        packages.append(
            (pkg["name"], pkg["version"], pkg.get("license") or "", os.path.dirname(pkg["manifest_path"]), authors, pkg.get("repository") or "")
        )
    return packages


def npm_packages():
    with open(os.path.join(ROOT, "package.json"), encoding="utf-8") as handle:
        top = json.load(handle).get("dependencies", {})
    seen = {}
    queue = list(top)
    while queue:
        name = queue.pop()
        if name in seen:
            continue
        directory = os.path.join(ROOT, "node_modules", *name.split("/"))
        manifest = os.path.join(directory, "package.json")
        if not os.path.isfile(manifest):
            continue
        with open(manifest, encoding="utf-8") as handle:
            data = json.load(handle)
        license_value = data.get("license")
        if not isinstance(license_value, str):
            license_value = json.dumps(license_value or data.get("licenses") or "")
        repo = data.get("repository")
        repo = repo.get("url") if isinstance(repo, dict) else repo
        author = data.get("author")
        author = author.get("name") if isinstance(author, dict) else author
        seen[name] = (name, data.get("version", "?"), license_value, directory, author or "", repo or "")
        queue.extend(data.get("dependencies", {}))
    return list(seen.values())


def collect():
    groups = defaultdict(list)  # texto -> [(ecosistema, nombre, versión, licencia)]
    missing = []  # (nombre, versión, licencia, autores, código fuente)
    count = 0
    for ecosystem, packages in (("Rust", rust_packages()), ("npm", npm_packages())):
        for name, version, spdx, directory, authors, repo in packages:
            count += 1
            texts = license_files(directory)
            if not texts:
                missing.append((name, version, spdx, authors, repo))
                continue
            for text in texts:
                groups[text].append((ecosystem, name, version, spdx))
    return count, groups, missing


def render(lang, count, groups, missing):
    s = STRINGS[lang]
    lines = [s["title"], ""] + s["intro"] + ["", f"{s['packages']}: {count}", ""]
    ordered = sorted(groups.items(), key=lambda item: (-len(item[1]), hashlib.sha1(item[0].encode()).hexdigest()))
    for text, owners in ordered:
        lines.append("=" * 78)
        for ecosystem, name, version, spdx in sorted(set(owners)):
            lines.append(f"{name} {version} [{ecosystem}] — {spdx or '?'}")
        lines.append("-" * 78)
        lines.append(text)
        lines.append("")
    if missing:
        lines.append("=" * 78)
        lines.extend(s["missing"])
        for name, version, spdx, authors, repo in sorted(missing):
            extra = "; ".join(
                part for part in ((f"{s['authors']}: {authors}") if authors else "", (f"{s['source']}: {repo}") if repo else "") if part
            )
            lines.append(f"  {name} {version} ({spdx or s['unknown']}){' — ' + extra if extra else ''}")
        lines.append("")
    lines.append("=" * 78)
    lines.extend(s["models"])
    lines.append("")
    return "\n".join(lines)


def main():
    count, groups, missing = collect()
    os.makedirs(LEGAL, exist_ok=True)
    for lang in STRINGS:
        out = os.path.join(LEGAL, f"THIRD_PARTY_LICENSES.{lang}.txt")
        with open(out, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(render(lang, count, groups, missing))
        print(f"{lang}: {os.path.getsize(out) / 1024:.0f} KB")
    print(f"{count} paquetes, {len(groups)} textos distintos, {len(missing)} sin archivo")


if __name__ == "__main__":
    sys.exit(main())
