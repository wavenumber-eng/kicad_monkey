"""Interactive ``textual`` TUI for ``kcr project create --tui``.

A single navigable form (not sequential prompts) — every field is editable in any
order, with an in-terminal directory/file browser. It returns a plain options dict
which the command turns into ``KiCadProjectCreateOptions`` for ``create_project``.
"""

from __future__ import annotations

from pathlib import Path

from kicad_monkey import KICAD_PAGE_SIZES, kicad_page_size_label
from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical, VerticalScroll
from textual.screen import ModalScreen
from textual.widget import Widget
from textual.widgets import (
    Button,
    DirectoryTree,
    Footer,
    Input,
    Label,
    Select,
    Static,
    Switch,
)

# (label, value) for the page-size dropdown — list + dimensions from kicad_monkey.
PAGE_SIZES: list[tuple[str, str]] = [
    (kicad_page_size_label(size), size) for size in KICAD_PAGE_SIZES
]


# --- Wavenumber "dark industrial" palette (house style) --------------------
WN_BG = "#0a0a0a"        # window background
WN_PANEL = "#0c0c0e"     # boxed panel fill
WN_FIELD = "#070709"     # field / console background
WN_BORDER = "#282832"    # 1px hairline border
WN_FG = "#e7dddd"        # text primary
WN_MUTED = "#969696"     # text muted
WN_ACCENT = "#b48700"    # amber accent
WN_ACCENT_HOVER = "#c89d00"
WN_SELECT = "#4a5aa0"    # selection
WN_ERR = "#ff6464"
WN_HOVER = "#1a1a1f"


class _PathPicker(ModalScreen[str | None]):
    """In-terminal directory/file browser. Returns the chosen path or None."""

    DEFAULT_CSS = f"""
    _PathPicker {{ align: center middle; background: {WN_BG} 70%; }}
    #picker {{
        width: 80%; height: 80%; padding: 1 2;
        background: {WN_PANEL}; border: solid {WN_BORDER};
    }}
    #picker-title {{
        height: 1; margin-bottom: 1; padding-left: 1;
        color: {WN_MUTED}; text-style: bold; border-left: thick {WN_ACCENT};
    }}
    #picker .navrow {{ height: auto; padding-bottom: 1; }}
    #picker #cwd {{ width: 1fr; color: {WN_MUTED}; padding-left: 2; content-align: left middle; }}
    #picker DirectoryTree {{
        height: 1fr; background: {WN_FIELD}; color: {WN_FG}; border: solid {WN_BORDER};
    }}
    #picker DirectoryTree:focus > .tree--cursor {{ background: {WN_SELECT}; color: {WN_FG}; }}
    #picker .sel {{ color: {WN_ACCENT}; padding: 1 0; }}
    #picker .row {{ height: auto; align: center middle; padding-top: 1; }}
    #picker .nav {{
        width: auto; min-width: 8; background: {WN_FIELD};
        color: {WN_FG}; border: solid {WN_BORDER};
    }}
    #picker .nav:hover {{ background: {WN_ACCENT}; color: {WN_BG}; }}
    #picker Button.primary {{
        background: {WN_ACCENT}; color: {WN_BG}; text-style: bold; border: solid {WN_ACCENT};
    }}
    #picker Button.primary:hover {{
        background: {WN_ACCENT_HOVER}; border: solid {WN_ACCENT_HOVER};
    }}
    #picker Button.secondary {{
        background: {WN_FIELD}; color: {WN_FG}; border: solid {WN_BORDER};
    }}
    #picker Button.secondary:hover {{ background: {WN_HOVER}; }}
    #picker .row Button {{ margin: 0 2; }}
    """

    def __init__(self, start: str, *, pick_files: bool) -> None:
        super().__init__()
        self._start = start if Path(start).is_dir() else str(Path.home())
        self._pick_files = pick_files
        self._selected: str | None = self._start

    def compose(self) -> ComposeResult:
        title = "CHOOSE A WORKSHEET (.wks)" if self._pick_files else "CHOOSE PROJECT DIRECTORY"
        with Vertical(id="picker"):
            yield Static(title, id="picker-title")
            with Horizontal(classes="navrow"):
                yield Button("▲ Up", id="up", classes="nav")
                yield Static(self._start, id="cwd")
            yield DirectoryTree(self._start, id="tree")
            yield Static(f"selected: {self._selected}", id="sel", classes="sel")
            with Horizontal(classes="row"):
                yield Button("Use this", id="use", classes="primary")
                yield Button("Cancel", id="cancel", classes="secondary")

    def _set_selected(self, path: str) -> None:
        self._selected = path
        self.query_one("#sel", Static).update(f"selected: {path}")

    def on_directory_tree_directory_selected(self, event: DirectoryTree.DirectorySelected) -> None:
        if not self._pick_files:
            self._set_selected(str(event.path))

    def on_directory_tree_file_selected(self, event: DirectoryTree.FileSelected) -> None:
        if self._pick_files:
            self._set_selected(str(event.path))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        event.stop()  # keep the press inside the modal (app's Cancel shares this id)
        if event.button.id == "up":
            tree = self.query_one("#tree", DirectoryTree)
            parent = Path(tree.path).parent
            tree.path = str(parent)
            tree.reload()
            self.query_one("#cwd", Static).update(str(parent))
            if not self._pick_files:
                self._set_selected(str(parent))
        elif event.button.id == "use":
            self.dismiss(self._selected)
        else:
            self.dismiss(None)


class ProjectCreateApp(App[dict | None]):
    """Single-form interactive editor for new-project options."""

    TITLE = "WAVENUMBER · kcr project create"
    CSS = f"""
    Screen {{ background: {WN_BG}; color: {WN_FG}; }}

    #titlebar {{
        height: 1; padding: 0 2; background: {WN_PANEL};
        color: {WN_ACCENT}; text-style: bold; border-bottom: solid {WN_BORDER};
    }}
    #form {{
        padding: 1 2; background: {WN_BG};
        scrollbar-background: {WN_BG}; scrollbar-background-hover: {WN_BG};
        scrollbar-color: {WN_BORDER}; scrollbar-color-hover: {WN_ACCENT};
        scrollbar-color-active: {WN_ACCENT_HOVER}; scrollbar-size-vertical: 1;
    }}

    /* boxed sections: 1px hairline panel + uppercase header w/ amber accent strip */
    .panel {{
        height: auto; padding: 0 2 1 2; margin-bottom: 1;
        background: {WN_PANEL}; border: solid {WN_BORDER};
    }}
    .panel-header {{
        height: 1; margin: 0 0 1 0; padding-left: 1;
        color: {WN_MUTED}; text-style: bold; border-left: thick {WN_ACCENT};
    }}
    .hint {{ color: {WN_MUTED}; height: 1; }}

    .field {{ height: auto; padding-bottom: 1; }}
    .field > Label {{ width: 18; content-align: left middle; color: {WN_MUTED}; }}

    Input, Select {{
        width: 1fr; background: {WN_FIELD}; color: {WN_FG}; border: solid {WN_BORDER};
    }}
    Input:focus, Select:focus {{ border: solid {WN_ACCENT}; }}
    Select SelectOverlay {{ background: {WN_PANEL}; border: solid {WN_BORDER}; }}
    Select SelectOverlay > .option-list--option-highlighted {{
        background: {WN_SELECT}; color: {WN_FG};
    }}

    .browse {{
        width: auto; min-width: 10; background: {WN_FIELD};
        color: {WN_FG}; border: solid {WN_BORDER};
    }}
    .browse:hover {{ background: {WN_ACCENT}; color: {WN_BG}; }}

    .toggle {{ height: auto; }}
    .toggle > Label {{ width: auto; padding-left: 1; color: {WN_FG}; content-align: left middle; }}
    Switch {{ background: {WN_FIELD}; border: solid {WN_BORDER}; }}
    Switch.-on {{ color: {WN_ACCENT}; }}
    Switch:focus {{ border: solid {WN_ACCENT}; }}

    /* key/value tables: text variables + symbol/footprint libraries */
    .kv-head {{ height: 1; }}
    .kv-head > Label {{ color: {WN_MUTED}; text-style: bold; }}
    .kv-col-a {{ width: 26; }}
    .kv-col-b {{ width: 1fr; }}
    .kv-col-x {{ width: 7; }}
    #var-rows, #symlib-rows, #fplib-rows {{ height: auto; }}
    .kv-row {{ height: auto; }}
    .kv-a {{ width: 26; background: {WN_FIELD}; color: {WN_FG}; border: solid {WN_BORDER}; }}
    .kv-b {{ width: 1fr; background: {WN_FIELD}; color: {WN_FG}; border: solid {WN_BORDER}; }}
    .kv-a:focus, .kv-b:focus {{ border: solid {WN_ACCENT}; }}
    .kv-remove {{
        width: 7; min-width: 5; background: {WN_FIELD};
        color: {WN_ERR}; border: solid {WN_BORDER};
    }}
    .kv-remove:hover {{ background: {WN_ERR}; color: {WN_BG}; }}
    .add-row {{ margin-top: 1; width: auto; }}

    #error {{ color: {WN_ERR}; height: auto; padding-left: 1; }}

    #actions {{ height: auto; align: center middle; padding-top: 1; }}
    #actions Button {{ margin: 0 3; }}
    Button.primary {{
        background: {WN_ACCENT}; color: {WN_BG}; text-style: bold; border: solid {WN_ACCENT};
    }}
    Button.primary:hover {{ background: {WN_ACCENT_HOVER}; border: solid {WN_ACCENT_HOVER}; }}
    Button.secondary {{ background: {WN_FIELD}; color: {WN_FG}; border: solid {WN_BORDER}; }}
    Button.secondary:hover {{ background: {WN_HOVER}; }}

    Footer {{ background: {WN_PANEL}; color: {WN_MUTED}; }}
    """

    def __init__(self, defaults: dict | None = None) -> None:
        super().__init__()
        self._d = defaults or {}

    def compose(self) -> ComposeResult:
        yield Static("◆ WAVENUMBER      ·      KCR PROJECT CREATE", id="titlebar")
        d = self._d
        with VerticalScroll(id="form"):
            with Vertical(classes="panel"):
                yield Static("NEW KICAD PROJECT", classes="panel-header")
                with Horizontal(classes="field"):
                    yield Label("Project name")
                    yield Input(value=d.get("name", ""), placeholder="My First Project", id="name")
                with Horizontal(classes="field"):
                    yield Label("Page size")
                    yield Select(
                        PAGE_SIZES, value=d.get("page_size", "D"), allow_blank=False, id="size",
                    )
                with Horizontal(classes="field"):
                    yield Label("Directory")
                    yield Input(value=d.get("directory", str(Path.cwd())), id="directory")
                    yield Button("Browse", id="browse_dir", classes="browse")
                with Horizontal(classes="field"):
                    yield Label("Top sheet name")
                    yield Input(
                        value=d.get("sheet_name", ""),
                        placeholder="(defaults to project name)", id="sheet",
                    )
                with Horizontal(classes="field"):
                    yield Label("Worksheet (.wks)")
                    yield Input(
                        value=d.get("worksheet", "") or "",
                        placeholder="(none — KiCad default)", id="worksheet",
                    )
                    yield Button("Browse", id="browse_wks", classes="browse")
                    yield Button("None", id="clear_wks", classes="browse")

            with Vertical(classes="panel"):
                yield Static("TITLE BLOCK", classes="panel-header")
                with Horizontal(classes="field"):
                    yield Label("Company")
                    yield Input(value=d.get("company", ""), id="company")
                with Horizontal(classes="field"):
                    yield Label("Revision")
                    yield Input(value=d.get("revision", ""), id="revision")
                with Horizontal(classes="field"):
                    yield Label("Date")
                    yield Input(value=d.get("date", ""), placeholder="e.g. 2026-06-22", id="date")

            with Vertical(classes="panel"):
                yield Static("OPTIONS", classes="panel-header")
                with Horizontal(classes="toggle"):
                    yield Switch(value=d.get("embed_worksheet", True), id="embed")
                    yield Label("Embed worksheet into the schematic")
                with Horizontal(classes="toggle"):
                    yield Switch(value=d.get("create_pcb", False), id="pcb")
                    yield Label("Also create a .kicad_pcb")
                with Horizontal(classes="toggle"):
                    yield Switch(value=d.get("create_lib_tables", True), id="libtables")
                    yield Label("Create empty sym-lib-table / fp-lib-table")

            with Vertical(classes="panel"):
                yield Static("PROJECT TEXT VARIABLES", classes="panel-header")
                with Horizontal(classes="kv-head"):
                    yield Label("NAME", classes="kv-col-a")
                    yield Label("VALUE", classes="kv-col-b")
                    yield Label("", classes="kv-col-x")
                yield Vertical(id="var-rows")
                yield Button("+ add variable", id="add_var", classes="browse add-row")

            with Vertical(classes="panel"):
                yield Static("SYMBOL LIBRARIES", classes="panel-header")
                with Horizontal(classes="kv-head"):
                    yield Label("NICKNAME", classes="kv-col-a")
                    yield Label("URI", classes="kv-col-b")
                    yield Label("", classes="kv-col-x")
                yield Vertical(id="symlib-rows")
                yield Button("+ add symbol library", id="add_symlib", classes="browse add-row")

            with Vertical(classes="panel"):
                yield Static("FOOTPRINT LIBRARIES", classes="panel-header")
                with Horizontal(classes="kv-head"):
                    yield Label("NICKNAME", classes="kv-col-a")
                    yield Label("URI", classes="kv-col-b")
                    yield Label("", classes="kv-col-x")
                yield Vertical(id="fplib-rows")
                yield Button("+ add footprint library", id="add_fplib", classes="browse add-row")

            yield Static("", id="error")
            with Horizontal(id="actions"):
                yield Button("CREATE PROJECT", id="create", classes="primary")
                yield Button("CANCEL", id="cancel", classes="secondary")
        yield Footer()

    def on_mount(self) -> None:
        d = self._d
        variables = d.get("text_variables") or {}
        for key, value in variables.items():
            self._add_kv_row("var-rows", "var-row", key, value, "NAME", "VALUE")
        if not variables:
            self._add_kv_row("var-rows", "var-row", a_ph="NAME", b_ph="VALUE")
        for lib in d.get("symbol_libraries") or []:
            self._add_kv_row(
                "symlib-rows", "symlib-row",
                lib.get("name", ""), lib.get("uri", ""), "NICKNAME", "URI",
            )
        for lib in d.get("footprint_libraries") or []:
            self._add_kv_row(
                "fplib-rows", "fplib-row",
                lib.get("name", ""), lib.get("uri", ""), "NICKNAME", "URI",
            )

    def _add_kv_row(
        self, container_id: str, row_class: str,
        a: str = "", b: str = "", a_ph: str = "", b_ph: str = "",
    ) -> None:
        row = Horizontal(
            Input(value=a, placeholder=a_ph, classes="kv-a"),
            Input(value=b, placeholder=b_ph, classes="kv-b"),
            Button("✕", classes="kv-remove"),
            classes=f"kv-row {row_class}",
        )
        self.query_one(f"#{container_id}", Vertical).mount(row)

    def _set_input(self, widget_id: str, value: str | None) -> None:
        if value:
            self.query_one(f"#{widget_id}", Input).value = value

    def _remove_row(self, button: Button) -> None:
        row = button.parent
        if isinstance(row, Widget):
            row.remove()

    def _browse(self, target_id: str, *, pick_files: bool) -> None:
        start = self.query_one("#directory", Input).value or str(Path.cwd())
        self.push_screen(
            _PathPicker(start, pick_files=pick_files),
            lambda p: self._set_input(target_id, p),
        )

    def on_button_pressed(self, event: Button.Pressed) -> None:
        bid = event.button.id
        add_specs = {
            "add_var": ("var-rows", "var-row", "NAME", "VALUE"),
            "add_symlib": ("symlib-rows", "symlib-row", "NICKNAME", "URI"),
            "add_fplib": ("fplib-rows", "fplib-row", "NICKNAME", "URI"),
        }
        if event.button.has_class("kv-remove"):
            self._remove_row(event.button)
        elif bid in add_specs:
            container, row_class, a_ph, b_ph = add_specs[bid]
            self._add_kv_row(container, row_class, a_ph=a_ph, b_ph=b_ph)
        elif bid == "clear_wks":
            self.query_one("#worksheet", Input).value = ""
        elif bid == "browse_dir":
            self._browse("directory", pick_files=False)
        elif bid == "browse_wks":
            self._browse("worksheet", pick_files=True)
        elif bid == "cancel":
            self.exit(None)
        elif bid == "create":
            self._submit()

    def _collect_libraries(self, row_class: str) -> list[dict[str, str]]:
        libs: list[dict[str, str]] = []
        for row in self.query(row_class):
            name = row.query_one(".kv-a", Input).value.strip()
            uri = row.query_one(".kv-b", Input).value.strip()
            if name and uri:
                libs.append({"name": name, "uri": uri})
        return libs

    def _submit(self) -> None:
        name = self.query_one("#name", Input).value.strip()
        if not name:
            self.query_one("#error", Static).update("Project name is required.")
            self.query_one("#name", Input).focus()
            return
        text_vars: dict[str, str] = {}
        for row in self.query(".var-row"):
            key = row.query_one(".kv-a", Input).value.strip()
            if key:
                text_vars[key] = row.query_one(".kv-b", Input).value

        worksheet = self.query_one("#worksheet", Input).value.strip() or None
        self.exit({
            "name": name,
            "directory": self.query_one("#directory", Input).value.strip() or ".",
            "page_size": self.query_one("#size", Select).value,
            "sheet_name": self.query_one("#sheet", Input).value.strip(),
            "worksheet": worksheet,
            "embed_worksheet": self.query_one("#embed", Switch).value,
            "create_pcb": self.query_one("#pcb", Switch).value,
            "create_lib_tables": self.query_one("#libtables", Switch).value,
            "text_variables": text_vars,
            "revision": self.query_one("#revision", Input).value.strip(),
            "company": self.query_one("#company", Input).value.strip(),
            "date": self.query_one("#date", Input).value.strip(),
            "symbol_libraries": self._collect_libraries(".symlib-row"),
            "footprint_libraries": self._collect_libraries(".fplib-row"),
        })


def run_project_create_tui(defaults: dict | None = None) -> dict | None:
    """Launch the form and return the chosen options dict, or None if cancelled."""
    return ProjectCreateApp(defaults).run()
