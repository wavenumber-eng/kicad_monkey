"""Typed targeted readers for KiCad S-expression files."""

from __future__ import annotations

from collections.abc import Callable, Iterator
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any, TypeVar, cast

from ._api_markers import public_api
from .kicad_sexpr import (
    SexpSelector,
    iter_sexp_file_form_spans,
    iter_sexp_form_spans,
    parse_sexp_span,
)

T = TypeVar("T")
SexpPredicate = Callable[[Any], bool]


@dataclass(frozen=True)
class _ReaderSpec:
    selector: SexpSelector
    predicate: SexpPredicate | None = None


def _selector_for_paths(*paths: tuple[str, ...]) -> SexpSelector:
    return SexpSelector(paths=paths)


def _has_child(sexp: Any, head: str) -> bool:
    return (
        isinstance(sexp, list)
        and any(isinstance(child, list) and child and child[0] == head for child in sexp)
    )


def _is_embedded_file_sexp(sexp: Any) -> bool:
    return (
        isinstance(sexp, list)
        and len(sexp) > 0
        and sexp[0] == "file"
        and _has_child(sexp, "name")
        and _has_child(sexp, "data")
    )


@lru_cache(maxsize=1)
def _default_reader_specs() -> dict[type[Any], _ReaderSpec]:
    from .kicad_footprint import KiCadFootprint
    from .kicad_lib_symbol import LibSymbol
    from .kicad_model import EmbeddedFile, Model
    from .kicad_pcb import KiCadPcb
    from .kicad_pcb_footprint import Footprint
    from .kicad_pcb_gr_arc import GrArc
    from .kicad_pcb_gr_circle import GrCircle
    from .kicad_pcb_gr_curve import GrCurve
    from .kicad_pcb_gr_line import GrLine
    from .kicad_pcb_gr_poly import GrPoly
    from .kicad_pcb_gr_rect import GrRect
    from .kicad_pcb_gr_text import GrText
    from .kicad_pcb_graphics import GrTextBox
    from .kicad_pcb_other import (
        Barcode,
        Dimension,
        GeneratedObject,
        Group,
        Image,
        Net,
        Table,
    )
    from .kicad_pcb_routing import Arc, Segment, Via
    from .kicad_pcb_zone import Zone
    from .kicad_sch_group import SchGroup
    from .kicad_sch_image import SchImage
    from .kicad_sch_junction import SchJunction
    from .kicad_sch_label import (
        SchGlobalLabel,
        SchHierarchicalLabel,
        SchLabel,
        SchNetclassFlag,
    )
    from .kicad_sch_no_connect import SchNoConnect
    from .kicad_sch_rule_area import SchRuleArea
    from .kicad_sch_shapes import SchArc, SchBezier, SchCircle, SchPolyline, SchRectangle
    from .kicad_sch_sheet import SchSheet
    from .kicad_sch_symbol import SchSymbol
    from .kicad_sch_table import SchTable
    from .kicad_sch_text import SchText
    from .kicad_sch_text_box import SchTextBox
    from .kicad_sch_wire import SchBus, SchBusAlias, SchBusEntry, SchWire
    from .kicad_schematic import KiCadSchematic
    from .kicad_symbol_lib import KiCadSymbolLib

    specs: dict[type[Any], _ReaderSpec] = {}

    def add(
        object_type: type[Any],
        *paths: tuple[str, ...],
        selector: SexpSelector | None = None,
        predicate: SexpPredicate | None = None,
    ) -> None:
        specs[object_type] = _ReaderSpec(
            selector=selector if selector is not None else _selector_for_paths(*paths),
            predicate=predicate,
        )

    # Whole-file roots.
    add(KiCadPcb, ("kicad_pcb",))
    add(KiCadSchematic, ("kicad_sch",))
    add(KiCadFootprint, ("footprint",))
    add(KiCadSymbolLib, ("kicad_symbol_lib",))

    # Library and placed component objects.
    add(Footprint, ("kicad_pcb", "footprint"), ("kicad_pcb", "module"))
    add(
        LibSymbol,
        ("kicad_symbol_lib", "symbol"),
        ("kicad_sch", "lib_symbols", "symbol"),
    )
    add(SchSymbol, ("kicad_sch", "symbol"))
    add(
        Model,
        ("kicad_pcb", "footprint", "model"),
        ("kicad_pcb", "module", "model"),
        ("footprint", "model"),
    )
    add(
        EmbeddedFile,
        selector=SexpSelector(heads={"file"}),
        predicate=_is_embedded_file_sexp,
    )

    # Board-level PCB objects used by rendering, health checks, and extraction.
    add(Net, ("kicad_pcb", "net"))
    add(GrText, ("kicad_pcb", "gr_text"))
    add(GrLine, ("kicad_pcb", "gr_line"))
    add(GrRect, ("kicad_pcb", "gr_rect"))
    add(GrArc, ("kicad_pcb", "gr_arc"))
    add(GrCircle, ("kicad_pcb", "gr_circle"))
    add(GrPoly, ("kicad_pcb", "gr_poly"))
    add(GrCurve, ("kicad_pcb", "gr_curve"))
    add(GrTextBox, ("kicad_pcb", "gr_text_box"))
    add(Barcode, ("kicad_pcb", "barcode"))
    add(Image, ("kicad_pcb", "image"))
    add(Table, ("kicad_pcb", "table"))
    add(Zone, ("kicad_pcb", "zone"))
    add(Dimension, ("kicad_pcb", "dimension"))
    add(Segment, ("kicad_pcb", "segment"))
    add(Via, ("kicad_pcb", "via"))
    add(Arc, ("kicad_pcb", "arc"))
    add(Group, ("kicad_pcb", "group"))
    add(GeneratedObject, ("kicad_pcb", "generated"))

    # Schematic top-level objects used by SVG/IR/netlist pipelines.
    add(SchWire, ("kicad_sch", "wire"))
    add(SchBus, ("kicad_sch", "bus"))
    add(SchBusEntry, ("kicad_sch", "bus_entry"))
    add(SchBusAlias, ("kicad_sch", "bus_alias"))
    add(SchJunction, ("kicad_sch", "junction"))
    add(SchNoConnect, ("kicad_sch", "no_connect"))
    add(SchLabel, ("kicad_sch", "label"))
    add(SchGlobalLabel, ("kicad_sch", "global_label"))
    add(SchHierarchicalLabel, ("kicad_sch", "hierarchical_label"))
    add(SchNetclassFlag, ("kicad_sch", "netclass_flag"))
    add(SchText, ("kicad_sch", "text"))
    add(SchTextBox, ("kicad_sch", "text_box"))
    add(SchPolyline, ("kicad_sch", "polyline"))
    add(SchRectangle, ("kicad_sch", "rectangle"))
    add(SchArc, ("kicad_sch", "arc"))
    add(SchCircle, ("kicad_sch", "circle"))
    add(SchBezier, ("kicad_sch", "bezier"))
    add(SchGroup, ("kicad_sch", "group"))
    add(SchImage, ("kicad_sch", "image"))
    add(SchRuleArea, ("kicad_sch", "rule_area"))
    add(SchTable, ("kicad_sch", "table"))
    add(SchSheet, ("kicad_sch", "sheet"))

    return specs


def _default_spec_for(object_type: type[Any]) -> _ReaderSpec:
    specs = _default_reader_specs()
    if object_type not in specs:
        raise ValueError(
            f"{object_type.__name__} has no default targeted reader selector; "
            "pass selector=... for custom S-expression object types"
        )
    return specs[object_type]


def _from_sexp_factory(object_type: type[T]) -> Callable[[Any], T]:
    factory = getattr(object_type, "from_sexp", None)
    if not callable(factory):
        raise TypeError(f"{object_type.__name__} does not define from_sexp(...)")
    return cast(Callable[[Any], T], factory)


@public_api
def iter_kicad_objects_from_text(
    text: str,
    object_type: type[T],
    *,
    selector: SexpSelector | None = None,
    predicate: SexpPredicate | None = None,
    source_path: str | Path | None = None,
) -> Iterator[T]:
    """Yield typed KiCad objects from selected forms without parsing the full file.

    ``object_type`` must expose ``from_sexp``. If ``selector`` is omitted, a
    built-in selector is used for known KiCad OOP classes such as ``Footprint``,
    ``LibSymbol``, ``SchSymbol``, ``Model``, and common PCB/schematic primitives.
    Pass ``selector`` when using this with a custom class or an uncommon form.
    """
    if selector is None:
        spec = _default_spec_for(object_type)
        active_selector = spec.selector
        spec_predicate = spec.predicate
    else:
        active_selector = selector
        spec_predicate = None
    factory = _from_sexp_factory(object_type)

    for span in iter_sexp_form_spans(text, active_selector, source_path=source_path):
        sexp = parse_sexp_span(span)
        if spec_predicate is not None and not spec_predicate(sexp):
            continue
        if predicate is not None and not predicate(sexp):
            continue
        yield factory(sexp)


@public_api
def iter_kicad_objects_from_file(
    path: str | Path,
    object_type: type[T],
    *,
    selector: SexpSelector | None = None,
    predicate: SexpPredicate | None = None,
) -> Iterator[T]:
    """Yield typed KiCad objects from selected forms in ``path``."""
    if selector is None:
        spec = _default_spec_for(object_type)
        active_selector = spec.selector
        spec_predicate = spec.predicate
    else:
        active_selector = selector
        spec_predicate = None
    factory = _from_sexp_factory(object_type)

    for span in iter_sexp_file_form_spans(path, active_selector):
        sexp = parse_sexp_span(span)
        if spec_predicate is not None and not spec_predicate(sexp):
            continue
        if predicate is not None and not predicate(sexp):
            continue
        yield factory(sexp)


__all__ = [
    "iter_kicad_objects_from_file",
    "iter_kicad_objects_from_text",
]
