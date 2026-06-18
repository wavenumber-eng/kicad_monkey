"""Source-span backed PCB projection API.

The projection API hydrates selected PCB sub-objects from exact KiCad source
forms without requiring a complete ``KiCadPcb`` parse.  Object-family methods
return the same domain classes used by the full PCB parser.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, Iterable, TypeVar, cast

from ._api_markers import public_api
from .kicad_base import unquote_string
from .kicad_model import EmbeddedFile, Model
from .kicad_pad import Pad
from .kicad_pcb_footprint import Footprint
from .kicad_pcb_graphics import GrTextBox
from .kicad_pcb_gr_arc import GrArc
from .kicad_pcb_gr_circle import GrCircle
from .kicad_pcb_gr_curve import GrCurve
from .kicad_pcb_gr_line import GrLine
from .kicad_pcb_gr_poly import GrPoly
from .kicad_pcb_gr_rect import GrRect
from .kicad_pcb_gr_text import GrText
from .kicad_pcb_other import (
    Barcode,
    BoardProperty,
    BoardVariant,
    Dimension,
    GeneratedObject,
    Group,
    Image,
    Layer,
    Net,
    NetRef,
    Stackup,
    Table,
    TitleBlock,
    UnknownElement,
)
from .kicad_pcb_routing import Arc, Segment, Via
from .kicad_pcb_zone import Zone
from .kicad_sexpr import SexpFormSpan, SexpSelector, iter_sexp_form_spans

if TYPE_CHECKING:
    from .kicad_pcb import KiCadPcb


T = TypeVar("T")


_PCB_KNOWN_TOP_LEVEL_HEADS = frozenset(
    {
        "kicad_pcb",
        "version",
        "generator",
        "generator_version",
        "general",
        "paper",
        "title_block",
        "layers",
        "setup",
        "net",
        "property",
        "variants",
        "gr_text",
        "gr_line",
        "gr_rect",
        "gr_arc",
        "gr_circle",
        "gr_poly",
        "gr_curve",
        "gr_text_box",
        "barcode",
        "image",
        "table",
        "footprint",
        "module",
        "zone",
        "dimension",
        "segment",
        "via",
        "arc",
        "group",
        "generated",
        "embedded_fonts",
        "embedded_files",
    }
)


@public_api
@dataclass(frozen=True)
class ProjectedSource:
    """Source metadata for one projected object."""

    span: SexpFormSpan | None
    parent_span: SexpFormSpan | None = None

    def text(self) -> str:
        """Return the exact source text for the projected object."""
        if self.span is None:
            raise ValueError("projected object has no source span")
        return self.span.text()

    def sexp(self) -> list:
        """Parse and return the exact projected source form."""
        if self.span is None:
            raise ValueError("projected object has no source span")
        parsed = self.span.parse()
        if not isinstance(parsed, list):
            raise ValueError("projected source did not parse to an S-expression form")
        return parsed


@public_api
@dataclass(frozen=True)
class PcbModelReference:
    """One footprint-owned 3D model reference from a projected board."""

    footprint: Footprint
    model: Model
    footprint_span: SexpFormSpan | None = None
    model_span: SexpFormSpan | None = None

    @property
    def reference(self) -> str:
        """Return the parent footprint reference, when present."""
        return self.footprint.get_property_value("Reference")

    @property
    def value(self) -> str:
        """Return the parent footprint value, when present."""
        return self.footprint.get_property_value("Value")

    @property
    def path(self) -> str:
        """Return the referenced model path."""
        return self.model.path


@public_api
class KiCadPcbProjection:
    """Lazy projection view over KiCad PCB source or a loaded ``KiCadPcb``."""

    def __init__(
        self,
        *,
        source_text: str | None = None,
        source_path: Path | None = None,
        board: "KiCadPcb | None" = None,
    ) -> None:
        if source_text is None and board is None:
            raise ValueError("KiCadPcbProjection requires source text or a board")
        self._source_text = source_text
        self.source_path = source_path
        self._board = board
        self._span_cache: dict[tuple[str, ...], list[SexpFormSpan]] = {}
        self._object_cache: dict[str, list[Any]] = {}
        self._single_cache: dict[str, Any] = {}
        self._source_by_id: dict[int, ProjectedSource] = {}

    @classmethod
    def from_file(cls, path: str | Path) -> "KiCadPcbProjection":
        """Create a projection from a PCB file."""
        source_path = Path(path)
        return cls(source_text=source_path.read_text(encoding="utf-8"), source_path=source_path)

    @classmethod
    def from_board(cls, board: "KiCadPcb") -> "KiCadPcbProjection":
        """Create a projection backed by an already parsed board."""
        raw_source_path = getattr(board, "source_path", None)
        source_path = Path(raw_source_path) if raw_source_path is not None else None
        source_text = None
        if source_path is not None and source_path.exists():
            source_text = source_path.read_text(encoding="utf-8")
        return cls(source_text=source_text, source_path=source_path, board=board)

    def source(self, obj: object) -> ProjectedSource | None:
        """Return source metadata for a projected object, when available."""
        return self._source_by_id.get(id(obj))

    def source_span(self, obj: object) -> SexpFormSpan | None:
        """Return the exact source span for a projected object, when available."""
        source = self.source(obj)
        return source.span if source is not None else None

    def source_text_for(self, obj: object) -> str | None:
        """Return exact source text for a projected object, when available."""
        source = self.source(obj)
        return source.text() if source is not None and source.span is not None else None

    def source_text_of(self, obj: object) -> str | None:
        """Alias for ``source_text_for``."""
        return self.source_text_for(obj)

    def source_text(self, obj: object) -> str | None:
        """Return exact source text for a projected object, when available."""
        return self.source_text_for(obj)

    def source_sexp(self, obj: object) -> list | None:
        """Return the source S-expression for a projected object."""
        source = self.source(obj)
        if source is not None and source.span is not None:
            return source.sexp()
        raw = getattr(obj, "_raw_sexp", None)
        return raw if isinstance(raw, list) else None

    def setup_sexp(self) -> list | None:
        """Return the board ``setup`` S-expression, when present."""
        if self._board is not None:
            return getattr(self._board, "setup_sexp", None)
        span = self._first_top_level_span("setup")
        if span is None:
            return None
        parsed = span.parse()
        return parsed if isinstance(parsed, list) else None

    def title_block(self) -> TitleBlock | None:
        """Return the board title block, when present."""
        if self._board is not None:
            obj = getattr(self._board, "title_block", None)
            self._register_single_board_source("title_block", obj)
            return obj
        return self._single_from_top_level("title_block", TitleBlock.from_sexp)

    def stackup(self) -> Stackup | None:
        """Return the board stackup, when present."""
        if self._board is not None:
            obj = getattr(self._board, "stackup", None)
            self._register_setup_child_board_source("stackup", obj)
            return obj
        if "stackup" in self._single_cache:
            return self._single_cache["stackup"]
        setup_span = self._first_top_level_span("setup")
        if setup_span is None:
            self._single_cache["stackup"] = None
            return None
        stackup_span = self._first_direct_child_span(setup_span, "stackup")
        if stackup_span is None:
            self._single_cache["stackup"] = None
            return None
        obj = Stackup.from_sexp(stackup_span.parse())
        self._register_source(obj, stackup_span, parent_span=setup_span)
        self._single_cache["stackup"] = obj
        return obj

    def embedded_fonts(self) -> bool:
        """Return whether board-level embedded fonts are enabled."""
        if self._board is not None:
            return bool(getattr(self._board, "embedded_fonts", False))
        span = self._first_top_level_span("embedded_fonts")
        if span is None:
            return False
        sexp = span.parse()
        return bool(isinstance(sexp, list) and len(sexp) > 1 and unquote_string(sexp[1]) == "yes")

    def layers(self) -> list[Layer]:
        return self._container_children("layers", "layers", Layer.from_sexp)

    def nets(self) -> list[Net]:
        return self._top_level_objects("nets", ("net",), Net.from_sexp)

    def properties(self) -> list[BoardProperty]:
        return self._top_level_objects("properties", ("property",), BoardProperty.from_sexp)

    def variants(self) -> list[BoardVariant]:
        return self._container_children("variants", "variants", BoardVariant.from_sexp, child_head="variant")

    def gr_texts(self) -> list[GrText]:
        return self._top_level_objects("gr_texts", ("gr_text",), GrText.from_sexp)

    def gr_lines(self) -> list[GrLine]:
        return self._top_level_objects("gr_lines", ("gr_line",), GrLine.from_sexp)

    def gr_rects(self) -> list[GrRect]:
        return self._top_level_objects("gr_rects", ("gr_rect",), GrRect.from_sexp)

    def gr_arcs(self) -> list[GrArc]:
        return self._top_level_objects("gr_arcs", ("gr_arc",), GrArc.from_sexp)

    def gr_circles(self) -> list[GrCircle]:
        return self._top_level_objects("gr_circles", ("gr_circle",), GrCircle.from_sexp)

    def gr_polys(self) -> list[GrPoly]:
        return self._top_level_objects("gr_polys", ("gr_poly",), GrPoly.from_sexp)

    def gr_curves(self) -> list[GrCurve]:
        return self._top_level_objects("gr_curves", ("gr_curve",), GrCurve.from_sexp)

    def gr_text_boxes(self) -> list[GrTextBox]:
        return self._top_level_objects("gr_text_boxes", ("gr_text_box",), GrTextBox.from_sexp)

    def images(self) -> list[Image]:
        return self._top_level_objects("images", ("image",), Image.from_sexp)

    def barcodes(self) -> list[Barcode]:
        return self._top_level_objects("barcodes", ("barcode",), Barcode.from_sexp)

    def tables(self) -> list[Table]:
        return self._top_level_objects("tables", ("table",), Table.from_sexp)

    def footprints(self) -> list[Footprint]:
        return self._top_level_objects(
            "footprints",
            ("footprint", "module"),
            Footprint.from_sexp,
            postprocess=self._resolve_footprint_pad_nets,
        )

    def zones(self) -> list[Zone]:
        return self._top_level_objects("zones", ("zone",), Zone.from_sexp, net_bound=True)

    def dimensions(self) -> list[Dimension]:
        return self._top_level_objects("dimensions", ("dimension",), Dimension.from_sexp)

    def segments(self) -> list[Segment]:
        return self._top_level_objects("segments", ("segment",), Segment.from_sexp, net_bound=True)

    def vias(self) -> list[Via]:
        return self._top_level_objects("vias", ("via",), Via.from_sexp, net_bound=True)

    def arcs(self) -> list[Arc]:
        return self._top_level_objects("arcs", ("arc",), Arc.from_sexp, net_bound=True)

    def groups(self) -> list[Group]:
        return self._top_level_objects("groups", ("group",), Group.from_sexp)

    def generated_items(self) -> list[GeneratedObject]:
        return self._top_level_objects("generated_items", ("generated",), GeneratedObject.from_sexp)

    def embedded_files(self) -> list[EmbeddedFile]:
        return self._container_children(
            "embedded_files",
            "embedded_files",
            EmbeddedFile.from_sexp,
            child_head="file",
        )

    def unknown_elements(self) -> list[UnknownElement]:
        """Return unknown top-level forms as ``UnknownElement`` objects."""
        if self._board is not None:
            return self._board_objects("unknown_elements", "unknown_elements")
        if "unknown_elements" in self._object_cache:
            return self._object_cache["unknown_elements"]
        objects: list[UnknownElement] = []
        for span in self._direct_root_child_spans():
            if span.head in _PCB_KNOWN_TOP_LEVEL_HEADS:
                continue
            sexp = span.parse()
            if isinstance(sexp, list) and sexp:
                obj = UnknownElement(name=str(sexp[0]), raw_sexp=sexp)
                self._register_source(obj, span)
                objects.append(obj)
        self._object_cache["unknown_elements"] = objects
        return objects

    def pads(self) -> list[Pad]:
        """Return footprint-owned pads as normal ``Pad`` objects."""
        if "pads" in self._object_cache:
            return self._object_cache["pads"]
        pads: list[Pad] = []
        for footprint in self.footprints():
            parent_span = self.source_span(footprint)
            pad_spans = self._direct_child_spans(parent_span, "pad") if parent_span else []
            for index, pad in enumerate(getattr(footprint, "pads", ())):
                if isinstance(getattr(pad, "net", None), NetRef):
                    pad.net = self._resolve_net_ref(pad.net)
                span = pad_spans[index] if index < len(pad_spans) else None
                self._register_source(pad, span, parent_span=parent_span)
                pads.append(pad)
        self._object_cache["pads"] = pads
        return pads

    def model_references(self) -> list[PcbModelReference]:
        """Return footprint-owned 3D model references with parent context."""
        if "model_references" in self._object_cache:
            return self._object_cache["model_references"]
        references: list[PcbModelReference] = []
        for footprint in self.footprints():
            footprint_span = self.source_span(footprint)
            model_spans = self._direct_child_spans(footprint_span, "model") if footprint_span else []
            for index, model in enumerate(getattr(footprint, "models", ())):
                model_span = model_spans[index] if index < len(model_spans) else None
                self._register_source(model, model_span, parent_span=footprint_span)
                references.append(
                    PcbModelReference(
                        footprint=footprint,
                        model=model,
                        footprint_span=footprint_span,
                        model_span=model_span,
                    )
                )
        self._object_cache["model_references"] = references
        return references

    def _top_level_objects(
        self,
        cache_key: str,
        heads: tuple[str, ...],
        factory: Callable[[list], T],
        *,
        net_bound: bool = False,
        postprocess: Callable[[T], None] | None = None,
    ) -> list[T]:
        if self._board is not None:
            return cast(list[T], self._board_objects(cache_key, cache_key, heads=heads))
        if cache_key in self._object_cache:
            return cast(list[T], self._object_cache[cache_key])
        objects: list[T] = []
        spans = self._top_level_spans(heads)
        for span in spans:
            obj = factory(span.parse())
            if net_bound:
                self._resolve_object_net(obj)
            if postprocess is not None:
                postprocess(obj)
            self._register_source(obj, span)
            objects.append(obj)
        self._object_cache[cache_key] = objects
        return objects

    def _container_children(
        self,
        cache_key: str,
        container_head: str,
        factory: Callable[[list], T],
        *,
        child_head: str | None = None,
    ) -> list[T]:
        if self._board is not None:
            return cast(
                list[T],
                self._board_objects(
                    cache_key,
                    cache_key,
                    container_head=container_head,
                    child_head=child_head,
                ),
            )
        if cache_key in self._object_cache:
            return cast(list[T], self._object_cache[cache_key])
        objects: list[T] = []
        container_span = self._first_top_level_span(container_head)
        if container_span is not None:
            for span in self._direct_child_spans(container_span, child_head):
                obj = factory(span.parse())
                self._register_source(obj, span, parent_span=container_span)
                objects.append(obj)
        self._object_cache[cache_key] = objects
        return objects

    def _board_objects(
        self,
        cache_key: str,
        board_attr: str,
        *,
        heads: tuple[str, ...] | None = None,
        container_head: str | None = None,
        child_head: str | None = None,
    ) -> list[Any]:
        if cache_key in self._object_cache:
            return self._object_cache[cache_key]
        if self._board is None:
            return []
        objects = list(getattr(self._board, board_attr, ()))
        spans: list[SexpFormSpan] = []
        parent_span: SexpFormSpan | None = None
        if self._source_text is not None:
            if heads is not None:
                spans = self._top_level_spans(heads)
            elif container_head is not None:
                parent_span = self._first_top_level_span(container_head)
                spans = self._direct_child_spans(parent_span, child_head) if parent_span is not None else []
        for index, obj in enumerate(objects):
            span = spans[index] if index < len(spans) else None
            self._register_source(obj, span, parent_span=parent_span)
        self._object_cache[cache_key] = objects
        return objects

    def _single_from_top_level(
        self,
        cache_key: str,
        factory: Callable[[list], T],
    ) -> T | None:
        if cache_key in self._single_cache:
            return self._single_cache[cache_key]
        span = self._first_top_level_span(cache_key)
        if span is None:
            self._single_cache[cache_key] = None
            return None
        obj = factory(span.parse())
        self._register_source(obj, span)
        self._single_cache[cache_key] = obj
        return obj

    def _register_single_board_source(self, head: str, obj: object | None) -> None:
        if obj is None or self._source_text is None:
            return
        self._register_source(obj, self._first_top_level_span(head))

    def _register_setup_child_board_source(self, head: str, obj: object | None) -> None:
        if obj is None or self._source_text is None:
            return
        setup_span = self._first_top_level_span("setup")
        if setup_span is None:
            return
        self._register_source(obj, self._first_direct_child_span(setup_span, head), parent_span=setup_span)

    def _resolve_footprint_pad_nets(self, footprint: Footprint) -> None:
        for pad in getattr(footprint, "pads", ()) or ():
            if isinstance(getattr(pad, "net", None), NetRef):
                pad.net = self._resolve_net_ref(pad.net)

    def _resolve_object_net(self, obj: object) -> None:
        net_ref = getattr(obj, "net", None)
        if isinstance(net_ref, NetRef):
            setattr(obj, "net", self._resolve_net_ref(net_ref))

    def _resolve_net_ref(self, net_ref: NetRef) -> NetRef:
        nets = self.nets()
        net_name_by_id = {net.ordinal: net.name for net in nets}
        net_id_by_name = {net.name: net.ordinal for net in nets}
        return net_ref.resolve_name(net_name_by_id).resolve_ordinal(net_id_by_name)

    def _register_source(
        self,
        obj: object,
        span: SexpFormSpan | None,
        *,
        parent_span: SexpFormSpan | None = None,
    ) -> None:
        self._source_by_id[id(obj)] = ProjectedSource(span=span, parent_span=parent_span)

    def _first_top_level_span(self, head: str) -> SexpFormSpan | None:
        spans = self._top_level_spans((head,))
        return spans[0] if spans else None

    def _top_level_spans(self, heads: Iterable[str]) -> list[SexpFormSpan]:
        head_tuple = tuple(heads)
        cache_key = tuple(sorted(head_tuple))
        if cache_key in self._span_cache:
            return self._span_cache[cache_key]
        head_set = set(head_tuple)
        spans = [
            span
            for span in self._top_level_indexed_spans()
            if span.head in head_set
        ]
        self._span_cache[cache_key] = spans
        return spans

    def _top_level_indexed_spans(self) -> list[SexpFormSpan]:
        return self._direct_root_child_spans()

    def _direct_root_child_spans(self) -> list[SexpFormSpan]:
        cache_key = ("<root-children>",)
        if cache_key in self._span_cache:
            return self._span_cache[cache_key]
        if self._source_text is None:
            return []
        spans = list(
            iter_sexp_form_spans(
                self._source_text,
                SexpSelector(min_depth=1, max_depth=1),
                source_path=self.source_path,
            )
        )
        self._span_cache[cache_key] = spans
        return spans

    def _direct_child_spans(
        self,
        parent_span: SexpFormSpan | None,
        head: str | None = None,
    ) -> list[SexpFormSpan]:
        if parent_span is None:
            return []
        cache_key = ("<children>", str(parent_span.start_offset), str(parent_span.end_offset), str(head))
        if cache_key in self._span_cache:
            return self._span_cache[cache_key]
        selector = SexpSelector(
            heads={head} if head is not None else None,
            min_depth=1,
            max_depth=1,
        )
        child_spans = [
            self._rebase_span(parent_span, child)
            for child in iter_sexp_form_spans(parent_span.text(), selector, source_path=self.source_path)
        ]
        self._span_cache[cache_key] = child_spans
        return child_spans

    def _first_direct_child_span(
        self,
        parent_span: SexpFormSpan,
        head: str,
    ) -> SexpFormSpan | None:
        spans = self._direct_child_spans(parent_span, head)
        return spans[0] if spans else None

    def _rebase_span(
        self,
        parent_span: SexpFormSpan,
        child_span: SexpFormSpan,
    ) -> SexpFormSpan:
        if self._source_text is None:
            return child_span
        start = parent_span.start_offset + child_span.start_offset
        end = parent_span.start_offset + child_span.end_offset
        line, column = _line_column_for_offset(self._source_text, start)
        end_line, end_column = _line_column_for_offset(self._source_text, end)
        return SexpFormSpan(
            head=child_span.head,
            path=parent_span.path + child_span.path[1:],
            depth=parent_span.depth + child_span.depth,
            start_offset=start,
            end_offset=end,
            line=line,
            column=column,
            end_line=end_line,
            end_column=end_column,
            source_text=self._source_text,
            source_path=str(self.source_path) if self.source_path is not None else None,
        )


def _line_column_for_offset(text: str, offset: int) -> tuple[int, int]:
    line = text.count("\n", 0, offset) + 1
    last_newline = text.rfind("\n", 0, offset)
    column = offset + 1 if last_newline < 0 else offset - last_newline
    return line, column


__all__ = [
    "KiCadPcbProjection",
    "PcbModelReference",
    "ProjectedSource",
]
