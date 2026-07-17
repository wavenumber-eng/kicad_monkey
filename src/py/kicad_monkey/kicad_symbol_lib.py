"""
KiCad symbol library (.kicad_sym) file parser.

This is the top-level parser for KiCad symbol library files.

Supports merging and splitting operations via class methods:
    # Merge all symbols from a directory
    lib = KiCadSymbolLib.from_directory('symbols/', recursive=True)
    lib.to_file('merged.kicad_sym')

    # Split a library into individual files
    lib = KiCadSymbolLib.from_file('merged.kicad_sym')
    count = lib.split_to_directory('output/', overwrite=True)
"""

from __future__ import annotations

import logging
from dataclasses import replace
from pathlib import Path
from typing import Iterable, Iterator, List, TYPE_CHECKING

from ._api_markers import public_api
from .kicad_defaults import (
    KICAD_GENERATOR_VERSION,
    KICAD_SYMBOL_LIB_FILE_VERSION,
    KICAD_SYMBOL_LIB_GENERATOR,
)
from .kicad_sexpr import parse_sexp, QuotedString, SexpWriter
from .kicad_base import find_all_elements, get_value, unquote_string

log = logging.getLogger(__name__)


@public_api
class KiCadSymbolLib:
    """KiCad symbol library (.kicad_sym) file.

    Example:
        >>> lib = KiCadSymbolLib("symbols.kicad_sym")
        >>> for symbol in lib.symbols:
        ...     print(symbol.name)
        >>> lib.save("output.kicad_sym")
    """

    def __init__(
        self,
        path: Path | str | None = None,
        *,
        version: int = KICAD_SYMBOL_LIB_FILE_VERSION,
        generator: str = KICAD_SYMBOL_LIB_GENERATOR,
        generator_version: str = KICAD_GENERATOR_VERSION,
        symbols: list['LibSymbol'] | None = None,
    ):
        """Create a KiCadSymbolLib.

        Args:
            path: Path to .kicad_sym file to parse.
                  If None, creates an empty library.
            version: KiCad symbol library file-format version.
            generator: KiCad generator name.
            generator_version: KiCad generator version string.
            symbols: Optional initial symbols.
        """
        self.version: int = version
        self.generator: str = generator
        self.generator_version: str = generator_version
        self.symbols: list['LibSymbol'] = list(symbols or [])
        self._raw_sexp = None

        if path is not None:
            parsed = self.from_file(Path(path))
            self.__dict__.update(parsed.__dict__)

    @classmethod
    @public_api
    def from_file(cls, path: Path | str) -> 'KiCadSymbolLib':
        """Load symbol library from file. Deprecated: use ``KiCadSymbolLib(path)``.

        Args:
            path: Path to .kicad_sym file
        """
        path = Path(path)
        text = path.read_text(encoding='utf-8')
        return cls.from_text(text)

    @classmethod
    @public_api
    def from_text(cls, text: str) -> 'KiCadSymbolLib':
        """Parse symbol library from text.

        Args:
            text: S-expression text content

        Returns:
            Parsed KiCadSymbolLib instance
        """
        sexp = parse_sexp(text)
        return cls.from_sexp(sexp)

    @classmethod
    @public_api
    def from_sexp(cls, sexp: list) -> 'KiCadSymbolLib':
        """Parse from S-expression list.

        Args:
            sexp: Parsed S-expression list

        Returns:
            KiCadSymbolLib instance
        """
        from .kicad_lib_symbol import LibSymbol

        # (kicad_symbol_lib (version N) (generator "...") (generator_version "...") (symbol ...))
        version = int(get_value(sexp, 'version', KICAD_SYMBOL_LIB_FILE_VERSION))
        generator = unquote_string(get_value(sexp, 'generator', KICAD_SYMBOL_LIB_GENERATOR))
        generator_version = unquote_string(get_value(sexp, 'generator_version', KICAD_GENERATOR_VERSION))

        symbols = []
        for sym_elem in find_all_elements(sexp, 'symbol'):
            symbols.append(LibSymbol.from_sexp(sym_elem))

        lib = cls(
            version=version,
            generator=generator,
            generator_version=generator_version,
            symbols=symbols,
        )
        lib._raw_sexp = sexp
        return lib

    @classmethod
    @public_api
    def from_files(cls, files: Iterable[Path | str]) -> 'KiCadSymbolLib':
        """Merge multiple symbol library files into a single library.

        The filename (without .kicad_sym) is used as the authoritative symbol name,
        which fixes issues where KiCad modifies names (e.g., strips leading zeros).

        Args:
            files: List of paths to .kicad_sym files

        Returns:
            New KiCadSymbolLib containing all symbols

        Example:
            >>> lib = KiCadSymbolLib.from_files(['sym1.kicad_sym', 'sym2.kicad_sym'])
            >>> lib.to_file('merged.kicad_sym')
        """
        file_paths = [Path(f) for f in files]

        all_symbols = []
        symbol_names: set[str] = set()

        # Track version info - use highest stable version
        max_version = KICAD_SYMBOL_LIB_FILE_VERSION
        max_generator_version = KICAD_GENERATOR_VERSION

        for file_path in file_paths:
            if not file_path.exists():
                log.warning(f"File not found, skipping: {file_path}")
                continue

            if file_path.suffix != '.kicad_sym':
                log.warning(f"Not a .kicad_sym file, skipping: {file_path.name}")
                continue

            try:
                lib = cls.from_file(file_path)
            except Exception as e:
                log.error(f"Failed to parse {file_path.name}: {e}")
                continue

            # Track max stable version (cap at V9, ignore V10 beta)
            if lib.version and lib.version <= KICAD_SYMBOL_LIB_FILE_VERSION:
                max_version = max(max_version, lib.version)

            if lib.generator_version:
                gen_ver = lib.generator_version
                if gen_ver.startswith('9.') and not gen_ver.startswith('9.99'):
                    if gen_ver > max_generator_version:
                        max_generator_version = gen_ver

            # Use filename as authoritative name for single-symbol libraries
            authoritative_name = file_path.stem

            for symbol in lib.symbols:
                original_name = symbol.name

                # Fix name mismatch (filename is authoritative)
                if len(lib.symbols) == 1 and original_name != authoritative_name:
                    old_name = symbol.name
                    symbol.name = authoritative_name

                    # Update subsymbol names
                    for subsym in symbol.subsymbols:
                        if subsym.name.startswith(old_name + '_'):
                            suffix = subsym.name[len(old_name):]
                            subsym.name = authoritative_name + suffix

                # Check for duplicates
                if symbol.name in symbol_names:
                    log.warning(f"Duplicate symbol '{symbol.name}' in {file_path.name}, skipping")
                    continue

                symbol_names.add(symbol.name)
                all_symbols.append(symbol)

        return cls(
            version=max_version,
            generator=KICAD_SYMBOL_LIB_GENERATOR,
            generator_version=max_generator_version,
            symbols=all_symbols
        )

    @classmethod
    @public_api
    def from_directory(
        cls,
        directory: Path | str,
        recursive: bool = True
    ) -> 'KiCadSymbolLib':
        """Merge all symbol library files from a directory into a single library.

        Args:
            directory: Directory containing .kicad_sym files
            recursive: If True, search subdirectories recursively

        Returns:
            New KiCadSymbolLib containing all symbols found

        Example:
            >>> lib = KiCadSymbolLib.from_directory('symbols/', recursive=True)
            >>> print(f"Merged {len(lib)} symbols")
            >>> lib.to_file('project_symbols.kicad_sym')
        """
        directory = Path(directory)

        if not directory.exists():
            log.error(f"Directory not found: {directory}")
            return cls()

        # Find all .kicad_sym files
        if recursive:
            symbol_files = sorted(directory.rglob("*.kicad_sym"))
        else:
            symbol_files = sorted(directory.glob("*.kicad_sym"))

        if not symbol_files:
            log.warning(f"No .kicad_sym files found in: {directory}")
            return cls()

        log.info(f"Found {len(symbol_files)} .kicad_sym file(s) in {directory}")

        return cls.from_files(symbol_files)

    @public_api
    def split_to_directory(
        self,
        output_dir: Path | str,
        overwrite: bool = False
    ) -> int:
        """Split this library into individual .kicad_sym files, one per symbol.

        Args:
            output_dir: Directory to save individual symbol files
            overwrite: If True, overwrite existing files

        Returns:
            Number of symbols extracted

        Example:
            >>> lib = KiCadSymbolLib.from_file('project_symbols.kicad_sym')
            >>> count = lib.split_to_directory('symbols/', overwrite=True)
            >>> print(f"Extracted {count} symbols")
        """
        output_dir = Path(output_dir)
        output_dir.mkdir(parents=True, exist_ok=True)

        extracted_count = 0
        for symbol in self.symbols:
            # Get clean symbol name (remove library prefix if present)
            symbol_name = symbol.name
            if ':' in symbol_name:
                symbol_name = symbol_name.split(':', 1)[1]

            # Sanitize filename
            safe_name = _sanitize_filename(symbol_name)
            output_file = output_dir / f"{safe_name}.kicad_sym"

            if output_file.exists() and not overwrite:
                log.info(f"Skipping '{symbol_name}' (file exists): {output_file.name}")
                continue

            # Create single-symbol library
            # Update symbol name to remove library prefix
            if ':' in symbol.name:
                old_prefix = symbol.name.split(':')[0] + ':'
                symbol.name = symbol_name
                # Update subsymbol names
                for subsym in symbol.subsymbols:
                    if subsym.name.startswith(old_prefix):
                        subsym.name = subsym.name[len(old_prefix):]

            single_lib = KiCadSymbolLib(
                version=self.version,
                generator=self.generator,
                generator_version=self.generator_version,
                symbols=[symbol]
            )

            try:
                single_lib.to_file(output_file)
                log.info(f"Extracted '{symbol_name}' -> {output_file.name}")
                extracted_count += 1
            except Exception as e:
                log.error(f"Failed to write '{symbol_name}': {e}")

        return extracted_count

    @public_api
    def save(self, path: Path | str) -> None:
        """Save symbol library to file. Canonical save method per ADR-0043.

        Args:
            path: Output file path
        """
        path = Path(path)
        text = self.to_text()
        path.write_text(text, encoding='utf-8')

    def to_file(self, path: Path | str) -> None:
        """Deprecated: use ``save()``."""
        self.save(path)

    def to_text(self) -> str:
        """Serialize to formatted S-expression text.

        Returns:
            Formatted S-expression string
        """
        return SexpWriter().write(self.to_sexp())

    def to_sexp(self) -> list:
        """Serialize to S-expression list.

        Returns:
            S-expression list structure
        """
        result = [
            'kicad_symbol_lib',
            ['version', self.version],
            ['generator', QuotedString(self.generator)],
            ['generator_version', QuotedString(self.generator_version)]
        ]

        for symbol in self.symbols:
            result.append(symbol.to_sexp())

        return result

    # Convenience methods
    @public_api
    def get_symbol(self, name: str) -> 'LibSymbol | None':
        """Get symbol by name.

        Args:
            name: Symbol name to find

        Returns:
            LibSymbol if found, None otherwise
        """
        for symbol in self.symbols:
            if symbol.name == name:
                return symbol
        return None

    @public_api
    def add_symbol(self, symbol: 'LibSymbol') -> None:
        """Add a symbol to the library.

        Args:
            symbol: Symbol to add
        """
        self.symbols.append(symbol)

    @public_api
    def remove_symbol(self, name: str) -> bool:
        """Remove symbol by name.

        Args:
            name: Symbol name to remove

        Returns:
            True if found and removed, False otherwise
        """
        for i, symbol in enumerate(self.symbols):
            if symbol.name == name:
                del self.symbols[i]
                return True
        return False

    @public_api
    def symbol_names(self) -> List[str]:
        """Get list of all symbol names.

        Returns:
            List of symbol name strings
        """
        return [s.name for s in self.symbols]

    @public_api
    def symbol_to_ir(
        self,
        symbol_name: str,
        *,
        unit: int | None = None,
        part_id: int | None = None,
        style: int = 0,
    ):
        """Render one symbol from this library to plotter IR.

        ``part_id`` is accepted as a compatibility alias for KiCad's
        ``unit`` selector. When neither is supplied, every unit matching
        ``style`` is included in the IR document.
        """
        from .kicad_lib_symbol_to_ir import lib_symbol_to_ir

        symbol = self.get_symbol(symbol_name)
        if symbol is None:
            raise ValueError(
                f"Symbol '{symbol_name}' not found in library. "
                f"Available symbols: {self.symbol_names()}"
            )
        symbol = _effective_render_symbol(self, symbol)
        selected_unit = _resolve_unit_alias(unit=unit, part_id=part_id)
        _validate_unit(symbol, selected_unit)
        return lib_symbol_to_ir(
            symbol,
            unit=selected_unit,
            style=style,
            document_id=symbol.name,
        )

    @public_api
    def to_ir(self, symbol_name: str, **kwargs):
        """Render one symbol from this library to plotter IR."""
        return self.symbol_to_ir(symbol_name, **kwargs)

    @public_api
    def symbol_to_svg(
        self,
        symbol_name: str,
        *,
        unit: int | None = None,
        part_id: int | None = None,
        style: int = 0,
        theme=None,
        options=None,
    ) -> str:
        """Render one symbol from this library as standalone SVG.

        ``part_id`` is a compatibility alias for ``unit``. Supplying both is
        allowed only when they match.
        If neither is supplied, unit 1 is rendered.
        """
        from .kicad_symbol_svg import (
            SymbolRenderOptions,
            SymbolTheme,
            render_symbol_svg,
        )

        symbol = self.get_symbol(symbol_name)
        if symbol is None:
            raise ValueError(
                f"Symbol '{symbol_name}' not found in library. "
                f"Available symbols: {self.symbol_names()}"
            )
        symbol = _effective_render_symbol(self, symbol)
        selected_unit = _resolve_unit_alias(unit=unit, part_id=part_id) or 1
        _validate_unit(symbol, selected_unit)

        render_options = options or SymbolRenderOptions(unit=selected_unit, style=style)
        if options is not None:
            render_options.unit = selected_unit
            render_options.style = style

        return render_symbol_svg(
            symbol,
            theme=theme or SymbolTheme(),
            options=render_options,
        )

    @public_api
    def to_svg(
        self,
        output_dir: Path | str | None = None,
        *,
        style: int = 0,
        theme=None,
    ) -> dict[str, dict[int, str]]:
        """Render every symbol/unit in the library to SVG.

        Returns ``{symbol_name: {unit: svg}}``. If ``output_dir`` is supplied,
        writes single-unit symbols as ``<symbol>_unit1.svg`` and multi-unit
        symbols as ``<symbol>_unitN.svg``.
        """
        out_dir = Path(output_dir) if output_dir is not None else None
        if out_dir is not None:
            out_dir.mkdir(parents=True, exist_ok=True)

        results: dict[str, dict[int, str]] = {}
        for symbol in self.symbols:
            results[symbol.name] = {}
            for unit in range(1, symbol.unit_count + 1):
                svg = self.symbol_to_svg(
                    symbol.name,
                    unit=unit,
                    style=style,
                    theme=theme,
                )
                results[symbol.name][unit] = svg
                if out_dir is not None:
                    filename = f"{_sanitize_filename(symbol.name)}_unit{unit}.svg"
                    (out_dir / filename).write_text(svg, encoding="utf-8")
        return results

    def __iter__(self) -> Iterator['LibSymbol']:
        """Iterate over symbols."""
        return iter(self.symbols)

    def __len__(self) -> int:
        """Number of symbols in library."""
        return len(self.symbols)

    def __getitem__(self, key: str | int) -> 'LibSymbol':
        """Get symbol by name or index.

        Args:
            key: Symbol name (str) or index (int)

        Returns:
            LibSymbol at index or with given name

        Raises:
            KeyError: If symbol name not found
            IndexError: If index out of range
        """
        if isinstance(key, int):
            return self.symbols[key]
        symbol = self.get_symbol(key)
        if symbol is None:
            raise KeyError(f"Symbol '{key}' not found")
        return symbol

    def __contains__(self, name: str) -> bool:
        """Check if symbol exists by name."""
        return self.get_symbol(name) is not None


if TYPE_CHECKING:
    from .kicad_lib_symbol import LibSymbol


def _resolve_unit_alias(*, unit: int | None, part_id: int | None) -> int | None:
    if unit is not None and part_id is not None and int(unit) != int(part_id):
        raise ValueError(f"unit ({unit}) and part_id ({part_id}) disagree")
    selected = unit if unit is not None else part_id
    if selected is None:
        return None
    selected_int = int(selected)
    if selected_int < 1:
        raise ValueError("unit/part_id must be >= 1")
    return selected_int


def _validate_unit(symbol: 'LibSymbol', unit: int | None) -> None:
    if unit is None:
        return
    if unit > symbol.unit_count:
        raise ValueError(
            f"unit {unit} exceeds symbol unit_count {symbol.unit_count} "
            f"for '{symbol.name}'"
        )


def _effective_render_symbol(
    library: KiCadSymbolLib,
    symbol: 'LibSymbol',
    *,
    _seen: set[str] | None = None,
) -> 'LibSymbol':
    """Return a symbol with inherited drawing primitives available for renderers."""
    if symbol.subsymbols or not symbol.extends:
        return symbol
    seen = set(_seen or set())
    if symbol.name in seen:
        return symbol
    seen.add(symbol.name)
    base = library.get_symbol(symbol.extends)
    if base is None:
        return symbol
    base = _effective_render_symbol(library, base, _seen=seen)
    if not base.subsymbols:
        return symbol
    return replace(
        symbol,
        pin_numbers_hide=base.pin_numbers_hide,
        pin_names_hide=base.pin_names_hide,
        pin_names_offset=base.pin_names_offset,
        has_demorgan_body_styles=base.has_demorgan_body_styles,
        body_style_names=list(base.body_style_names),
        subsymbols=list(base.subsymbols),
        embedded_fonts=list(base.embedded_fonts),
    )


def _sanitize_filename(name: str) -> str:
    """Sanitize a symbol name to be a valid filename."""
    # Replace invalid filename characters with underscore
    invalid_chars = r'<>:"/\|?*'
    for char in invalid_chars:
        name = name.replace(char, '_')
    # Also replace whitespace
    for char in ' \t\n\r':
        name = name.replace(char, '_')
    return name


__all__ = ['KiCadSymbolLib']
