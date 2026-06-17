"""
Subtest: Filter Framework
Stratum: L2_tools
Purpose: Lock down current behavior of the kicad_monkey sexpr-level filter
framework before refactoring (Phase C-B).

Covers:
- KiCadFilterPipeline file-to-file entry point methods
- 14 individual filter functions (sexpr→sexpr)
- 1 formatting helper (format_kicad_sexp)

Tests use synthetic in-memory inputs by design — the filters operate on
parsed s-expressions, so we don't need real corpus files for most cases.
The entry-point round-trip tests use temp files to exercise the file IO
path. Heavy-geometry filters (add_fab_bounding_orthogonal_convex,
orthographic_projection_outline) are exercised on minimal pad fixtures so
the numpy/shapely/trimesh code paths are at least smoke-tested.

Note: parse_sexp returns the parsed list directly (already-unwrapped).
The filters iterate this list looking for top-level child elements of
interest (`symbol`, `lib_symbols`, `layers`, etc.).
"""

from __future__ import annotations

import tempfile
from pathlib import Path
from textwrap import dedent

import pytest

# Phase E Slice E-3 — gate the whole filter framework suite on its
# heavy geometry deps. ``kicad_filter_footprint`` imports ``numpy``,
# ``shapely``, and ``trimesh`` at module level; without them the entire
# filter chain (entry points + every individual filter) fails to import.
# These are declared as hard runtime deps in ``kicad_monkey/pyproject``,
# but minimal dev environments may not have them — skip cleanly rather
# than emitting 25 noisy failures.
pytest.importorskip(
    "numpy",
    reason="kicad_monkey filter framework requires numpy (declared as a "
           "hard dep in kicad_monkey/pyproject.toml)",
)
pytest.importorskip(
    "shapely",
    reason="kicad_monkey filter framework requires shapely",
)
pytest.importorskip(
    "trimesh",
    reason="kicad_monkey filter framework requires trimesh",
)

from kicad_monkey.kicad_base import find_all_elements, find_element
from kicad_monkey.kicad_sexpr import QuotedString, parse_sexp


# ============================================================================
# Module Import Tests
# ============================================================================

def test_module_imports():
    """All filter framework symbols import via the public surface."""
    import kicad_monkey as km

    # Entry points
    assert callable(km.KiCadFilterPipeline)
    # Helpers
    assert callable(km.format_kicad_sexp)
    # Individual filters
    assert callable(km.fp_filter__clean_fab)
    assert callable(km.fp_filter__clean_layers)
    assert callable(km.fp_filter__add_fab_bounding_orthogonal_convex)
    assert callable(km.fp_filter__fix_zero_sized_pads)
    assert callable(km.fp_filter__fix_fp_text_font_to_arial)
    assert callable(km.fp_filter__normalized_embedded_model_naming)
    assert callable(km.fp_filter__orthographic_projection_outline)
    assert callable(km.fp_filter__remove_schematic_inherited_metadata)
    assert callable(km.pcb_filter__reset_layer_user_names)
    assert callable(km.pcb_filter__process_embedded_footprints)
    assert callable(km.sch_filter__remove_altium_value_property)
    assert callable(km.sym_filter__clear_property_values)
    assert callable(km.sym_filter__remove_nonstandard_properties)
    assert callable(km.sym_filter__standardize_reference_value_fonts)


# ============================================================================
# Test helpers
# ============================================================================

def _property_names(symbol: list) -> list[str]:
    """Extract property names from a (symbol ...) sexpr."""
    return [
        str(p[1]).strip('"')
        for p in symbol
        if isinstance(p, list) and p and p[0] == 'property'
    ]


def _property_value(symbol: list, name: str) -> str | None:
    """Get a property's value (3rd element) from a (symbol ...) sexpr."""
    for p in symbol:
        if isinstance(p, list) and len(p) >= 3 and p[0] == 'property':
            if str(p[1]).strip('"') == name:
                return str(p[2]).strip('"')
    return None


# ============================================================================
# Symbol filters
# ============================================================================

class TestSymFilterRemoveNonstandardProperties:
    """sym_filter__remove_nonstandard_properties keeps only Reference, Value,
    Description, Footprint, Datasheet on each symbol."""

    def test_removes_nonstandard_properties(self):
        from kicad_monkey import sym_filter__remove_nonstandard_properties
        sexp = parse_sexp(dedent("""
            (kicad_symbol_lib (version 20241209)
                (symbol "Foo"
                    (property "Reference" "U")
                    (property "Value" "Foo")
                    (property "Description" "test")
                    (property "Footprint" "")
                    (property "Datasheet" "")
                    (property "Altium_Designator" "U1")
                    (property "Vendor" "ACME")
                )
            )
        """))
        out = sym_filter__remove_nonstandard_properties(sexp)
        symbol = find_element(out, 'symbol')
        assert _property_names(symbol) == [
            'Reference', 'Value', 'Description', 'Footprint', 'Datasheet'
        ]

    def test_no_change_when_only_standard(self):
        from kicad_monkey import sym_filter__remove_nonstandard_properties
        sexp = parse_sexp(dedent("""
            (kicad_symbol_lib (version 20241209)
                (symbol "Foo"
                    (property "Reference" "U")
                    (property "Value" "Foo")
                )
            )
        """))
        out = sym_filter__remove_nonstandard_properties(sexp)
        symbol = find_element(out, 'symbol')
        assert _property_names(symbol) == ['Reference', 'Value']


class TestSymFilterClearPropertyValues:
    """sym_filter__clear_property_values empties Value/Description/Footprint/
    Datasheet, leaves Reference alone."""

    def test_clears_non_reference_values(self):
        from kicad_monkey import sym_filter__clear_property_values
        sexp = parse_sexp(dedent("""
            (kicad_symbol_lib (version 20241209)
                (symbol "Foo"
                    (property "Reference" "U")
                    (property "Value" "Foo")
                    (property "Description" "test description")
                    (property "Footprint" "lib:fp")
                    (property "Datasheet" "http://ex.com")
                )
            )
        """))
        out = sym_filter__clear_property_values(sexp)
        symbol = find_element(out, 'symbol')
        assert _property_value(symbol, 'Reference') == 'U'
        assert _property_value(symbol, 'Value') == ''
        assert _property_value(symbol, 'Description') == ''
        assert _property_value(symbol, 'Footprint') == ''
        assert _property_value(symbol, 'Datasheet') == ''


class TestSymFilterStandardizeFonts:
    """sym_filter__standardize_reference_value_fonts forces Arial 2.1844 bold
    on Reference and Arial 1.524 on Value."""

    def test_overrides_fonts(self):
        from kicad_monkey import sym_filter__standardize_reference_value_fonts
        sexp = parse_sexp(dedent("""
            (kicad_symbol_lib (version 20241209)
                (symbol "Foo"
                    (property "Reference" "U" (effects (font (size 1.27 1.27))))
                    (property "Value" "Foo" (effects (font (size 1.27 1.27))))
                    (property "Datasheet" "" (effects (font (size 1.27 1.27))))
                )
            )
        """))
        out = sym_filter__standardize_reference_value_fonts(sexp)
        symbol = find_element(out, 'symbol')
        for prop in symbol:
            if not (isinstance(prop, list) and prop and prop[0] == 'property'):
                continue
            name = str(prop[1]).strip('"')
            effects = find_element(prop, 'effects')
            if effects is None:
                continue
            font = find_element(effects, 'font')
            assert font is not None, f"no font on {name}"
            face = find_element(font, 'face')
            size = find_element(font, 'size')
            if name == 'Reference':
                assert face == ['face', QuotedString('Arial')]
                assert size == ['size', 2.1844, 2.1844]
                assert ['bold', 'yes'] in font
            elif name == 'Value':
                assert face == ['face', QuotedString('Arial')]
                assert size == ['size', 1.524, 1.524]
            elif name == 'Datasheet':
                # Should be untouched
                assert size == ['size', 1.27, 1.27]


# ============================================================================
# Footprint metadata filters
# ============================================================================

class TestFpFilterRemoveSchematicInheritedMetadata:
    """Footprint cleanup removes symbol/board-instance metadata only."""

    def test_removes_inherited_properties_and_instance_attrs(self):
        from kicad_monkey import fp_filter__remove_schematic_inherited_metadata

        sexp = parse_sexp(dedent("""
            (footprint "Header"
                (version 20260206)
                (generator "pcbnew")
                (generator_version "10.0")
                (layer "F.Cu")
                (property "Reference" "REF**")
                (property "Value" "1x6 Header")
                (property "Datasheet" "")
                (property "Description" "Generic header")
                (property "Manufacturer" "ACME")
                (property "cad-reference" "100_1_6")
                (property ki_fp_filters "wavenumber:100_1_6")
                (attr smd exclude_from_bom dnp exclude_from_pos_files)
                (pad "1" thru_hole rect
                    (at -6.35 0)
                    (size 1.5748 1.5748)
                    (drill 0.9652)
                    (layers "*.Cu" "*.Mask")
                    (remove_unused_layers no)
                )
            )
        """))

        out = fp_filter__remove_schematic_inherited_metadata(sexp)

        assert _property_names(out) == ["Reference", "Value"]
        assert find_element(out, "attr") == ["attr", "smd"]
        pad = find_element(out, "pad")
        assert find_element(pad, "remove_unused_layers") == ["remove_unused_layers", "no"]


# ============================================================================
# Schematic filter
# ============================================================================

class TestSchFilterRemoveAltiumValue:
    def test_removes_altium_value_property(self):
        from kicad_monkey import sch_filter__remove_altium_value_property
        sexp = parse_sexp(dedent("""
            (kicad_sch (version 20211123)
                (lib_symbols
                    (symbol "lib:Foo"
                        (property "Reference" "U")
                        (property "ALTIUM_VALUE" "stale")
                        (property "Value" "Foo")
                    )
                )
            )
        """))
        out = sch_filter__remove_altium_value_property(sexp)
        lib_symbols = find_element(out, 'lib_symbols')
        symbol = find_element(lib_symbols, 'symbol')
        names = _property_names(symbol)
        assert 'ALTIUM_VALUE' not in names
        assert names == ['Reference', 'Value']

    def test_no_change_when_no_altium_value(self):
        from kicad_monkey import sch_filter__remove_altium_value_property
        sexp = parse_sexp(dedent("""
            (kicad_sch (version 20211123)
                (lib_symbols
                    (symbol "lib:Foo"
                        (property "Reference" "U")
                        (property "Value" "Foo")
                    )
                )
            )
        """))
        out = sch_filter__remove_altium_value_property(sexp)
        lib_symbols = find_element(out, 'lib_symbols')
        symbol = find_element(lib_symbols, 'symbol')
        assert _property_names(symbol) == ['Reference', 'Value']


# ============================================================================
# PCB filters
# ============================================================================

class TestPcbFilterResetLayerUserNames:
    def test_strips_user_name_field(self):
        from kicad_monkey import pcb_filter__reset_layer_user_names
        sexp = parse_sexp(dedent("""
            (kicad_pcb
                (layers
                    (0 "F.Cu" signal "Top Layer")
                    (5 "F.SilkS" user "Top Overlay")
                    (31 "B.Cu" signal)
                )
            )
        """))
        out = pcb_filter__reset_layer_user_names(sexp)
        layers = find_element(out, 'layers')
        # 4-tuples become 3-tuples
        assert layers[1] == [0, QuotedString('F.Cu'), 'signal']
        assert layers[2] == [5, QuotedString('F.SilkS'), 'user']
        # Already-3-tuple stays as is
        assert layers[3] == [31, QuotedString('B.Cu'), 'signal']


class TestPcbFilterProcessEmbeddedFootprints:
    def test_runs_footprint_chain_on_each_embedded_fp(self):
        from kicad_monkey import pcb_filter__process_embedded_footprints
        sexp = parse_sexp(dedent("""
            (kicad_pcb (version 20241229) (generator "pcbnew")
                (footprint "lib:R0402"
                    (layer "F.Cu")
                    (pad "1" smd rect (at 0 0) (size 0 0) (layers "F.Cu"))
                    (fp_text user "REF**" (at 0 0) (layer "F.Fab"))
                )
                (footprint "lib:C0805"
                    (layer "F.Cu")
                    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu"))
                )
            )
        """))
        out = pcb_filter__process_embedded_footprints(sexp)
        fps = find_all_elements(out, 'footprint')
        assert len(fps) == 2
        # The R0402 footprint had a zero-size pad; should be fixed.
        sizes_in_r0402 = []
        for pad in find_all_elements(fps[0], 'pad'):
            size = find_element(pad, 'size')
            if size:
                sizes_in_r0402.append((size[1], size[2]))
        assert (0.001, 0.001) in sizes_in_r0402


# ============================================================================
# Footprint filters
# ============================================================================

def _layers_used(footprint: list) -> list[str]:
    """List layer names of fp_line elements in a footprint."""
    layers = []
    for fp_line in find_all_elements(footprint, 'fp_line'):
        layer = find_element(fp_line, 'layer')
        if layer:
            layers.append(str(layer[1]).strip('"'))
    return layers


class TestFpFilterCleanLayers:
    def test_default_removes_fab_and_user_layers(self):
        from kicad_monkey import fp_filter__clean_fab
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (fp_line (start 0 0) (end 1 1) (layer "F.Fab"))
                (fp_line (start 0 0) (end 1 1) (layer "F.SilkS"))
                (fp_line (start 0 0) (end 1 1) (layer "User.1"))
                (fp_line (start 0 0) (end 1 1) (layer "Eco1.User"))
            )
        """))
        out = fp_filter__clean_fab(sexp)
        assert _layers_used(out) == ['F.SilkS']

    def test_custom_layer_list(self):
        from kicad_monkey import fp_filter__clean_layers
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (fp_line (start 0 0) (end 1 1) (layer "F.SilkS"))
                (fp_line (start 0 0) (end 1 1) (layer "B.SilkS"))
            )
        """))
        out = fp_filter__clean_layers(sexp, layers=["F.SilkS"])
        assert _layers_used(out) == ['B.SilkS']


class TestFpFilterFixZeroSizedPads:
    def test_replaces_zero_size_with_1um(self):
        from kicad_monkey import fp_filter__fix_zero_sized_pads
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (pad "1" smd rect (at 0 0) (size 0 0) (layers "F.Cu"))
                (pad "2" smd rect (at 1 0) (size 0.5 0.5) (layers "F.Cu"))
            )
        """))
        out = fp_filter__fix_zero_sized_pads(sexp)
        sizes = []
        for pad in find_all_elements(out, 'pad'):
            size = find_element(pad, 'size')
            if size:
                sizes.append((size[1], size[2]))
        assert sizes == [(0.001, 0.001), (0.5, 0.5)]


class TestFpFilterFixFontToArial:
    def test_replaces_existing_font_face(self):
        from kicad_monkey import fp_filter__fix_fp_text_font_to_arial
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (fp_text user "REF**" (at 0 0) (layer "F.Fab")
                    (effects (font (face "Helvetica") (size 1 1)))
                )
            )
        """))
        out = fp_filter__fix_fp_text_font_to_arial(sexp)
        text = find_element(out, 'fp_text')
        face = find_element(find_element(find_element(text, 'effects'), 'font'), 'face')
        assert face[1] == QuotedString('Arial')

    def test_adds_face_when_missing(self):
        from kicad_monkey import fp_filter__fix_fp_text_font_to_arial
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (fp_text user "REF**" (at 0 0) (layer "F.Fab")
                    (effects (font (size 1 1)))
                )
            )
        """))
        out = fp_filter__fix_fp_text_font_to_arial(sexp)
        text = find_element(out, 'fp_text')
        face = find_element(find_element(find_element(text, 'effects'), 'font'), 'face')
        assert face is not None and face[1] == QuotedString('Arial')

    def test_adds_effects_when_missing(self):
        from kicad_monkey import fp_filter__fix_fp_text_font_to_arial
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (fp_text user "REF**" (at 0 0) (layer "F.Fab"))
            )
        """))
        out = fp_filter__fix_fp_text_font_to_arial(sexp)
        text = find_element(out, 'fp_text')
        effects = find_element(text, 'effects')
        assert effects is not None
        font = find_element(effects, 'font')
        face = find_element(font, 'face')
        assert face[1] == QuotedString('Arial')


class TestFpFilterNormalizedEmbeddedModelNaming:
    def test_renames_embedded_model_to_match_footprint(self):
        from kicad_monkey import fp_filter__normalized_embedded_model_naming
        sexp = parse_sexp(dedent("""
            (footprint "MyPart"
                (model "kicad-embed://Wrong.STEP" (offset (xyz 0 0 0)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 0)))
                (embedded_files
                    (file (name "Wrong.STEP") (type "model"))
                )
            )
        """))
        out = fp_filter__normalized_embedded_model_naming(sexp)
        ef = find_element(out, 'embedded_files')
        model = find_element(out, 'model')
        # Embedded file's nested (name "...") inside the (file ...) record
        file_elem = find_element(ef, 'file')
        name = find_element(file_elem, 'name')
        assert name[1] == QuotedString('MyPart.STEP')
        assert model[1] == QuotedString('kicad-embed://MyPart.STEP')

    def test_strips_pcb_lib_prefix(self):
        from kicad_monkey import fp_filter__normalized_embedded_model_naming
        sexp = parse_sexp(dedent("""
            (footprint "lib:MyPart"
                (model "kicad-embed://Wrong.STEP")
                (embedded_files
                    (file (name "Wrong.STEP") (type "model"))
                )
            )
        """))
        out = fp_filter__normalized_embedded_model_naming(sexp)
        ef = find_element(out, 'embedded_files')
        file_elem = find_element(ef, 'file')
        name = find_element(file_elem, 'name')
        # PCB-style "lib:name" — only the suffix is used
        assert name[1] == QuotedString('MyPart.STEP')


class TestFpFilterAddFabBoundingOrthogonalConvex:
    """Heavy-geometry filter: exercises numpy path. Needs >= 4 corners worth
    of points (a full convex hull) — single-pad inputs are degenerate."""

    def test_adds_fab_outline_around_pads(self):
        from kicad_monkey import fp_filter__add_fab_bounding_orthogonal_convex
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (layer "F.Cu")
                (pad "1" smd rect (at -2 -1) (size 0.5 0.5) (layers "F.Cu"))
                (pad "2" smd rect (at  2 -1) (size 0.5 0.5) (layers "F.Cu"))
                (pad "3" smd rect (at -2  1) (size 0.5 0.5) (layers "F.Cu"))
                (pad "4" smd rect (at  2  1) (size 0.5 0.5) (layers "F.Cu"))
            )
        """))
        out = fp_filter__add_fab_bounding_orthogonal_convex(sexp)
        # New fp_line entries on F.Fab should now exist
        fab_lines = []
        for fp_line in find_all_elements(out, 'fp_line'):
            layer = find_element(fp_line, 'layer')
            if layer and str(layer[1]).strip('"') == 'F.Fab':
                fab_lines.append(fp_line)
        assert len(fab_lines) > 0


class TestFpFilterOrthographicProjectionOutline:
    """Heavy-geometry filter: trimesh+shapely+zstd code path. Without an
    embedded STEP model, falls back to add_fab_bounding_orthogonal_convex
    so the test verifies the fallback runs cleanly and pads are preserved."""

    def test_falls_back_when_no_embedded_step(self):
        from kicad_monkey import fp_filter__orthographic_projection_outline
        sexp = parse_sexp(dedent("""
            (footprint "lib:Foo"
                (layer "F.Cu")
                (pad "1" smd rect (at -2 -1) (size 0.5 0.5) (layers "F.Cu"))
                (pad "2" smd rect (at  2 -1) (size 0.5 0.5) (layers "F.Cu"))
                (pad "3" smd rect (at -2  1) (size 0.5 0.5) (layers "F.Cu"))
                (pad "4" smd rect (at  2  1) (size 0.5 0.5) (layers "F.Cu"))
            )
        """))
        out = fp_filter__orthographic_projection_outline(sexp)
        pads = find_all_elements(out, 'pad')
        assert len(pads) == 4


# ============================================================================
# Formatting helpers
# ============================================================================

class TestFormatKicadSexp:
    def test_produces_tab_indented_output(self):
        from kicad_monkey import format_kicad_sexp
        sexp = ['kicad_pcb', ['version', 20241229], ['generator', QuotedString('pcbnew')]]
        out = format_kicad_sexp(sexp)
        assert 'kicad_pcb' in out
        assert '20241229' in out
        # Indented lines should start with tabs, not spaces
        for line in out.split('\n')[1:]:
            if line and line[0] in (' ', '\t'):
                assert line[0] == '\t', f"line not tab-indented: {line!r}"


# ============================================================================
# Entry-point file-IO round-trips
# ============================================================================

class TestKicadFpFilterEntryPoint:
    def test_roundtrips_minimal_footprint_file(self):
        """End-to-end: filter_footprint reads a .kicad_mod, applies all
        footprint filters, writes output."""
        from kicad_monkey import KiCadFilterPipeline

        with tempfile.TemporaryDirectory() as td:
            in_path = Path(td) / "Foo.kicad_mod"
            out_path = Path(td) / "Foo.out.kicad_mod"
            in_path.write_text(dedent("""
                (footprint "Foo" (version 20241229) (generator "pcbnew") (layer "F.Cu")
                    (pad "1" smd rect (at -2 -1) (size 0.5 0.5) (layers "F.Cu"))
                    (pad "2" smd rect (at  2 -1) (size 0 0)     (layers "F.Cu"))
                    (pad "3" smd rect (at -2  1) (size 0.5 0.5) (layers "F.Cu"))
                    (pad "4" smd rect (at  2  1) (size 0.5 0.5) (layers "F.Cu"))
                    (fp_text user "REF**" (at 0 0) (layer "F.Fab"))
                )
            """).strip(), encoding='utf-8')
            KiCadFilterPipeline().filter_footprint(in_path, out_path)
            text = out_path.read_text(encoding='utf-8')
            # zero-size pad fixed
            assert "(size 0.001 0.001" in text
            # Fab outline regenerated
            assert "F.Fab" in text


class TestKicadSymFilterEntryPoint:
    def test_roundtrips_minimal_symbol_lib(self):
        from kicad_monkey import KiCadFilterPipeline

        with tempfile.TemporaryDirectory() as td:
            in_path = Path(td) / "lib.kicad_sym"
            out_path = Path(td) / "lib.out.kicad_sym"
            in_path.write_text(dedent("""
                (kicad_symbol_lib (version 20241209) (generator "kicad_symbol_editor")
                    (symbol "Foo"
                        (property "Reference" "U" (at 0 0 0) (effects (font (size 1.27 1.27))))
                        (property "Value" "Foo" (at 0 -2 0) (effects (font (size 1.27 1.27))))
                        (property "Vendor" "ACME" (at 0 -4 0) (effects (font (size 1.27 1.27))))
                    )
                )
            """).strip(), encoding='utf-8')
            KiCadFilterPipeline().filter_symbol(in_path, out_path)
            text = out_path.read_text(encoding='utf-8')
            # Vendor (nonstandard) removed
            assert '"Vendor"' not in text
            # Reference font standardized
            assert "Arial" in text
            # Value cleared
            assert '(property "Value" ""' in text


class TestKicadSchFilterEntryPoint:
    def test_roundtrips_minimal_schematic(self):
        from kicad_monkey import KiCadFilterPipeline

        with tempfile.TemporaryDirectory() as td:
            in_path = Path(td) / "test.kicad_sch"
            out_path = Path(td) / "test.out.kicad_sch"
            in_path.write_text(dedent("""
                (kicad_sch (version 20211123) (generator "eeschema")
                    (lib_symbols
                        (symbol "lib:Foo"
                            (property "Reference" "U" (at 0 0 0))
                            (property "ALTIUM_VALUE" "stale" (at 0 -2 0))
                            (property "Value" "Foo" (at 0 -4 0))
                        )
                    )
                )
            """).strip(), encoding='utf-8')
            KiCadFilterPipeline().filter_schematic(in_path, out_path)
            text = out_path.read_text(encoding='utf-8')
            assert "ALTIUM_VALUE" not in text
            assert '(property "Value"' in text


class TestKicadPcbFilterEntryPoint:
    def test_roundtrips_minimal_pcb(self):
        from kicad_monkey import KiCadFilterPipeline

        with tempfile.TemporaryDirectory() as td:
            in_path = Path(td) / "test.kicad_pcb"
            out_path = Path(td) / "test.out.kicad_pcb"
            in_path.write_text(dedent("""
                (kicad_pcb (version 20241229) (generator "pcbnew")
                    (layers
                        (0 "F.Cu" signal "Top Layer")
                        (31 "B.Cu" signal "Bottom Layer")
                    )
                    (footprint "lib:Foo" (layer "F.Cu")
                        (pad "1" smd rect (at -2 -1) (size 0 0)     (layers "F.Cu"))
                        (pad "2" smd rect (at  2 -1) (size 0.5 0.5) (layers "F.Cu"))
                        (pad "3" smd rect (at -2  1) (size 0.5 0.5) (layers "F.Cu"))
                        (pad "4" smd rect (at  2  1) (size 0.5 0.5) (layers "F.Cu"))
                    )
                )
            """).strip(), encoding='utf-8')
            KiCadFilterPipeline().filter_pcb(in_path, out_path, reset_layer_names=True)
            text = out_path.read_text(encoding='utf-8')
            # Layer user names stripped (opt-in via reset_layer_names=True)
            assert "Top Layer" not in text
            assert "Bottom Layer" not in text
            # Zero-size pad fixed on the embedded footprint
            assert "(size 0.001 0.001" in text

    def test_layer_names_kept_by_default(self):
        """reset_layer_names defaults to False — layer user names are preserved."""
        from kicad_monkey import KiCadFilterPipeline

        with tempfile.TemporaryDirectory() as td:
            in_path = Path(td) / "test.kicad_pcb"
            out_path = Path(td) / "test.out.kicad_pcb"
            in_path.write_text(dedent("""
                (kicad_pcb (version 20241229) (generator "pcbnew")
                    (layers
                        (0 "F.Cu" signal "Top Layer")
                    )
                )
            """).strip(), encoding='utf-8')
            KiCadFilterPipeline().filter_pcb(in_path, out_path)
            text = out_path.read_text(encoding='utf-8')
            assert "Top Layer" in text


# ============================================================================
# Run tests standalone
# ============================================================================

if __name__ == "__main__":
    pytest.main([__file__, "-v"])
