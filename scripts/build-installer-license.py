"""Genera src-tauri/installer/license.txt, la pantalla de licencia del instalador.

El instalador de Tauri muestra un único archivo de licencia, así que se junta el
acuerdo en español e inglés. Se guarda en UTF-8 SIN BOM: Tauri le antepone él
mismo el BOM de UTF-8 al copiarlo al instalador, y NSIS lo lee así (un archivo
UTF-16 queda con dos BOM y la pantalla muestra basura). Los 6 idiomas completos
viajan aparte en `legal/`.

Uso (desde la raíz del repo):  python scripts/build-installer-license.py
"""

import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEPARATOR = "\r\n\r\n" + "-" * 60 + "\r\n\r\n"


def read(lang):
    with open(os.path.join(ROOT, "legal", f"LICENSE_NOTICE.{lang}.txt"), encoding="utf-8") as handle:
        return handle.read().strip().replace("\r\n", "\n").replace("\n", "\r\n")


def main():
    text = read("es") + SEPARATOR + read("en") + "\r\n"
    out = os.path.join(ROOT, "src-tauri", "installer", "license.txt")
    with open(out, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)
    print(f"{len(text)} caracteres -> {out}")


if __name__ == "__main__":
    main()
