"""Rack ownership for the Rust symbol-library reader/writer slice."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess

from jsonschema import Draft202012Validator
import msgspec

from kicad_monkey import KiCadSymbolLib
from kicad_monkey.contracts.generated import decode_symbol_library_read_result_a0


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = (
    PACKAGE_ROOT / "contracts" / "generated" / "schema" / "SymbolLibraryReadResult.json"
)
SOURCE = """# retained comment
(kicad_symbol_lib
  (version 20231120)
  (generator kicad_symbol_editor)
  (symbol "Base"
    (property "Reference" "U")
    (in_bom yes)
    (on_board no)
    (symbol "Base_1_1"
      (pin input line (at 0 0 0) (length 2.54) (name "A") (number "1"))))
  (symbol "Derived"
    (extends "Base")
    (power local)
    (symbol "Derived_1_1"
      (pin power_in inverted (at 0 0 90) (length 2.54) (name "VCC") (number "2"))
      (pin power_in line (at 0 0 270) (length 2.54) (name "GND") (number "3"))))
)"""


def _python_summaries() -> list[dict]:
    library = KiCadSymbolLib.from_text(SOURCE)
    return [
        {
            "name": symbol.name,
            **({"extends": symbol.extends} if symbol.extends is not None else {}),
            "in_bom": symbol.in_bom,
            "on_board": symbol.on_board,
            "power": symbol.power,
            **({"power_kind": symbol.power_kind} if symbol.power_kind is not None else {}),
            "property_count": len(symbol.properties),
            "subsymbol_count": len(symbol.subsymbols),
            "pin_count": sum(len(subsymbol.pins) for subsymbol in symbol.subsymbols),
        }
        for symbol in library.symbols
    ]


def test_python_summary_oracle_and_generated_contract_are_exact() -> None:
    expected = [
        {
            "name": "Base", "in_bom": True, "on_board": False, "power": False,
            "property_count": 1, "subsymbol_count": 1, "pin_count": 1,
        },
        {
            "name": "Derived", "extends": "Base", "in_bom": True,
            "on_board": True, "power": True, "power_kind": "local",
            "property_count": 0, "subsymbol_count": 1, "pin_count": 2,
        },
    ]
    assert _python_summaries() == expected
    payload = {
        "type": "kicad_monkey.symbol_library_read.result",
        "version": "a0",
        "source_bytes": str(len(SOURCE.encode())),
        "symbols": expected,
        "diagnostics": [],
    }
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(payload)
    decoded = decode_symbol_library_read_result_a0(json.dumps(payload).encode())
    assert json.loads(msgspec.json.encode(decoded)) == payload


def test_rack_orchestrates_native_host_and_real_wasm_writeback() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust symbol-library gate"
    commands = [
        [cargo, "test", "--locked", "--package", "kicad-monkey-core", "--test", "symbol_library_typed_slice"],
        [cargo, "test", "--locked", "--package", "kicad-monkey-wasm"],
        [cargo, "test", "--locked", "--package", "kicad-monkey-wasm", "--target", "wasm32-unknown-unknown"],
    ]
    for command in commands:
        completed = subprocess.run(
            command, cwd=PACKAGE_ROOT, capture_output=True, text=True, timeout=120, check=False
        )
        assert completed.returncode == 0, (
            f"Command failed: {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
