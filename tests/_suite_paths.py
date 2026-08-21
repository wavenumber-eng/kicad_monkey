from __future__ import annotations

import sys
from pathlib import Path

from corpus_output_paths import (
    TEST_GENERATED_CORPUS_ROOT as TEST_GENERATED_CORPUS_ROOT,
    resolve_test_corpus_output_path as resolve_test_corpus_output_path,
)

TESTS_DIR = Path(__file__).resolve().parent
KICAD_PACKAGE_ROOT = TESTS_DIR.parent
WORKSPACE_ROOT = KICAD_PACKAGE_ROOT.parent
KICAD_MODULE_ROOT = KICAD_PACKAGE_ROOT / "src" / "py" / "kicad_monkey"
TESTS_REPO_ROOT = KICAD_PACKAGE_ROOT
SOURCE_ROOT = KICAD_PACKAGE_ROOT / "src" / "py"
if str(SOURCE_ROOT) not in sys.path:
    sys.path.insert(0, str(SOURCE_ROOT))

from kicad_monkey.testing import corpus as _corpus  # noqa: E402


TEST_CORPUS_ARCHIVE = _corpus.DEFAULT_CORPUS_ARCHIVE
TEST_CORPUS_DIR = _corpus.DEFAULT_CORPUS_AUTHORING_ROOT
TEST_CORPUS_UNPACKED_DIR = _corpus.DEFAULT_CORPUS_CACHE_ROOT
TEST_CORPUS_ROOT = _corpus.get_test_corpus_root()
def ensure_import_paths() -> None:
    paths = [TESTS_DIR, KICAD_PACKAGE_ROOT / "src" / "py"]
    for path in paths:
        path_text = str(path)
        if path_text not in sys.path:
            sys.path.insert(0, path_text)
