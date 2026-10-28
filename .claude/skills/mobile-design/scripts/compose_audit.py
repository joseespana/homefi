#!/usr/bin/env python3
"""
Compose / Android audit for the HomeFi app (Kotlin + Jetpack Compose).

The bundled mobile_audit.py only understands React Native / Flutter; this script
applies the same mobile-design principles to Kotlin sources and Android resources.

Usage: python3 compose_audit.py [app_dir]     (default: apps/android/app)
Exit code 1 when an ERROR is found.
"""

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ERRORS: list[str] = []
WARNINGS: list[str] = []
PASSED: list[str] = []


def rel(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def line_of(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def check_kotlin(file: Path, root: Path) -> None:
    text = file.read_text(encoding="utf-8", errors="replace")
    name = rel(file, root)
    in_theme = "/ui/theme/" in file.as_posix()

    # 1. Icon-only buttons need a content description (TalkBack)
    for m in re.finditer(r"IconButton\([^)]*\)\s*\{\s*Icon\(\s*[^,()]+(?:\([^)]*\))?\s*,\s*null", text):
        ERRORS.append(f"[A11y] {name}:{line_of(text, m.start())} IconButton icon has contentDescription = null")

    # 2. Hard-coded user-visible text (must live in strings.xml for EN + ES)
    for m in re.finditer(r'\bText\(\s*"((?:[^"\\]|\\.)*)"', text):
        literal = re.sub(r"\$\{[^}]*\}|\$\w+", "", m.group(1))
        if len(re.findall(r"[A-Za-zÁÉÍÓÚáéíóúñÑ]", literal)) > 1:
            WARNINGS.append(f"[i18n] {name}:{line_of(text, m.start())} hard-coded text \"{m.group(1)[:40]}\" -> stringResource")

    # 3. Colors belong to the theme
    if not in_theme:
        for m in re.finditer(r"\bColor\(0x[0-9A-Fa-f]{6,8}\)", text):
            WARNINGS.append(f"[Color] {name}:{line_of(text, m.start())} hard-coded color, use MaterialTheme.colorScheme / Brand")
    for m in re.finditer(r"Color\(0xFF000000\)|background\(Color\.Black\)", text):
        WARNINGS.append(f"[Color] {name}:{line_of(text, m.start())} pure black surface (use the dark surface token)")

    # 4. Text sizes in dp ignore the user's font scale
    for m in re.finditer(r"fontSize\s*=\s*\d+(?:\.\d+)?\.dp", text):
        ERRORS.append(f"[Type] {name}:{line_of(text, m.start())} fontSize in dp, use sp / typography styles")

    # 5. Lists need stable keys
    for m in re.finditer(r"\bitems\(\s*[^,)]+\)\s*\{", text):
        WARNINGS.append(f"[Perf] {name}:{line_of(text, m.start())} LazyColumn items() without key =")

    # 6. Lifecycle-aware collection
    for m in re.finditer(r"\.collectAsState\(\)", text):
        WARNINGS.append(f"[Perf] {name}:{line_of(text, m.start())} collectAsState() -> collectAsStateWithLifecycle()")

    # 7. Threading / scope sins
    for m in re.finditer(r"\brunBlocking\b", text):
        ERRORS.append(f"[Thread] {name}:{line_of(text, m.start())} runBlocking in app code blocks the main thread")
    for m in re.finditer(r"\bGlobalScope\b", text):
        WARNINGS.append(f"[Scope] {name}:{line_of(text, m.start())} GlobalScope, use appScope / viewModelScope")

    # 8. Secrets in logs
    for m in re.finditer(r"Log\.[dviwe]\([^)]*(token|password|secret|refresh)", text, re.IGNORECASE):
        ERRORS.append(f"[Security] {name}:{line_of(text, m.start())} logging credentials")

    # 9. Editing generated bindings is never right
    if "uniffi" in file.as_posix() and "generated" not in file.as_posix() and "uniffiRustCall" in text:
        ERRORS.append(f"[FFI] {name}: copy of generated UniFFI code in sources; consume the generated bindings instead")


def string_keys(path: Path) -> set[str]:
    if not path.exists():
        return set()
    return {e.get("name") for e in ET.parse(path).getroot() if e.tag in ("string", "plurals", "string-array")}


def check_strings(res: Path) -> None:
    base = string_keys(res / "values" / "strings.xml")
    if not base:
        WARNINGS.append("[i18n] res/values/strings.xml not found")
        return
    for locale_dir in sorted(res.glob("values-*")):
        strings = locale_dir / "strings.xml"
        if not strings.exists():
            continue
        keys = string_keys(strings)
        missing = sorted(base - keys - {"app_name"})
        extra = sorted(keys - base)
        if missing:
            ERRORS.append(f"[i18n] {locale_dir.name}/strings.xml missing: {', '.join(missing)}")
        if extra:
            WARNINGS.append(f"[i18n] {locale_dir.name}/strings.xml has unknown keys: {', '.join(extra)}")
        if not missing and not extra:
            PASSED.append(f"[i18n] {locale_dir.name} matches values/ ({len(base)} keys)")


def check_manifest(manifest: Path) -> None:
    if not manifest.exists():
        return
    text = manifest.read_text(encoding="utf-8")
    if 'android:allowBackup="true"' in text:
        ERRORS.append("[Security] allowBackup=true would back up auth.json")
    if "usesCleartextTraffic=\"true\"" in text:
        ERRORS.append("[Security] cleartext traffic enabled")
    if "foregroundServiceType" in text and "FOREGROUND_SERVICE_DATA_SYNC" not in text:
        ERRORS.append("[Platform] foreground service type declared without its FOREGROUND_SERVICE_* permission")
    icon = manifest.parent / "res/mipmap-anydpi-v26/ic_launcher.xml"
    if not icon.exists() or "monochrome" not in icon.read_text(encoding="utf-8", errors="ignore"):
        WARNINGS.append("[Icon] adaptive icon without <monochrome> layer (Android 13 themed icons)")
    PASSED.append("[Manifest] checked")


def main() -> int:
    app = Path(sys.argv[1] if len(sys.argv) > 1 else "apps/android/app").resolve()
    main_dir = app / "src" / "main"
    if not main_dir.exists():
        print(f"not an Android app module: {app}")
        return 2

    files = [f for f in main_dir.rglob("*.kt") if "/build/" not in f.as_posix()]
    for f in files:
        check_kotlin(f, app)
    check_strings(main_dir / "res")
    check_manifest(main_dir / "AndroidManifest.xml")

    print(f"[COMPOSE AUDIT] {len(files)} Kotlin files in {app}")
    print("-" * 60)
    for e in ERRORS:
        print(f"ERROR   {e}")
    for w in WARNINGS:
        print(f"WARN    {w}")
    for p in PASSED:
        print(f"OK      {p}")
    print("-" * 60)
    print(f"{len(ERRORS)} errors, {len(WARNINGS)} warnings -> {'FAIL' if ERRORS else 'PASS'}")
    return 1 if ERRORS else 0


if __name__ == "__main__":
    sys.exit(main())
