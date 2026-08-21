use super::{KiCadLibPart, KiCadNet, KiCadNetlist, KiCadNetlistComponent};
use crate::{SourceBundleError, SourceBundleErrorKind};
use std::collections::HashSet;
use std::fmt::Write as _;

const NETLIST_VERSION: &str = "E";

pub fn emit_kicad_netlist(
    netlist: &KiCadNetlist,
    source_path: &str,
    date: &str,
    tool: &str,
    max_output_bytes: usize,
) -> Result<String, SourceBundleError> {
    let mut output = PrettyEmitter::new(max_output_bytes);
    output.open("export")?;
    output.quoted_form("version", NETLIST_VERSION)?;
    emit_design(&mut output, netlist, source_path, date, tool)?;
    let included = emit_components(&mut output, &netlist.components)?;
    emit_libparts(&mut output, &netlist.libparts)?;
    output.open("libraries")?;
    for library in &netlist.libraries {
        output.open("library")?;
        output.quoted_form("logical", library)?;
        output.close()?;
    }
    output.close()?;
    emit_nets(&mut output, &netlist.nets, &included)?;
    output.close()?;
    output.finish()
}

fn emit_design(
    output: &mut PrettyEmitter,
    netlist: &KiCadNetlist,
    source_path: &str,
    date: &str,
    tool: &str,
) -> Result<(), SourceBundleError> {
    output.open("design")?;
    output.quoted_form("source", source_path)?;
    output.quoted_form("date", date)?;
    output.quoted_form("tool", tool)?;
    for sheet in &netlist.sheets {
        output.open("sheet")?;
        output.quoted_form("number", &sheet.number.to_string())?;
        output.quoted_form("name", nonempty(&sheet.name, "/"))?;
        output.quoted_form("tstamps", nonempty(&sheet.tstamps, "/"))?;
        output.open("title_block")?;
        output.optional_quoted_form("title", &sheet.title)?;
        output.optional_quoted_form("company", &sheet.company)?;
        output.optional_quoted_form("rev", &sheet.revision)?;
        output.optional_quoted_form("date", &sheet.date)?;
        output.quoted_form("source", "")?;
        for number in 1..=9 {
            output.open("comment")?;
            output.quoted_form("number", &number.to_string())?;
            output.quoted_form("value", "")?;
            output.close()?;
        }
        output.close()?;
        output.close()?;
    }
    output.close()
}

fn emit_components<'a>(
    output: &mut PrettyEmitter,
    components: &'a [KiCadNetlistComponent],
) -> Result<HashSet<&'a str>, SourceBundleError> {
    output.open("components")?;
    let mut included = HashSet::with_capacity(components.len());
    for component in components.iter().filter(|component| component.on_board) {
        included.insert(component.reference.as_str());
        emit_component(output, component)?;
    }
    output.close()?;
    Ok(included)
}

fn emit_component(
    output: &mut PrettyEmitter,
    component: &KiCadNetlistComponent,
) -> Result<(), SourceBundleError> {
    output.open("comp")?;
    output.quoted_form("ref", &component.reference)?;
    output.quoted_form("value", nonempty(&component.value, "~"))?;
    output.nonempty_form("footprint", &component.footprint)?;
    output.nonempty_form("datasheet", &component.datasheet)?;
    output.nonempty_form("description", &component.description)?;
    output.open("fields")?;
    for (name, value) in &component.fields {
        output.open("field")?;
        output.quoted_form("name", name)?;
        if !value.is_empty() {
            output.quoted(value)?;
        }
        output.close()?;
    }
    output.close()?;
    output.open("libsource")?;
    output.quoted_form("lib", &component.libsource_lib)?;
    output.quoted_form("part", &component.libsource_part)?;
    output.quoted_form("description", &component.libsource_description)?;
    output.close()?;
    for (name, value) in &component.properties {
        output.open("property")?;
        output.quoted_form("name", name)?;
        output.quoted_form("value", value)?;
        output.close()?;
    }
    output.open("sheetpath")?;
    output.quoted_form("names", nonempty(&component.sheet_path_names, "/"))?;
    output.quoted_form("tstamps", nonempty(&component.sheet_path_uuids, "/"))?;
    output.close()?;
    if !component.instance_uuids.is_empty() {
        output.open("tstamps")?;
        for uuid in &component.instance_uuids {
            output.quoted(uuid)?;
        }
        output.close()?;
    }
    output.open("units")?;
    for unit in &component.units {
        output.open("unit")?;
        output.quoted_form("name", &unit.name)?;
        output.open("pins")?;
        for pin in &unit.pins {
            output.open("pin")?;
            output.quoted_form("num", pin)?;
            output.close()?;
        }
        output.close()?;
        output.close()?;
    }
    output.close()?;
    output.close()
}

fn emit_libparts(
    output: &mut PrettyEmitter,
    libparts: &[KiCadLibPart],
) -> Result<(), SourceBundleError> {
    output.open("libparts")?;
    for part in libparts {
        emit_libpart(output, part)?;
    }
    output.close()
}

fn emit_libpart(output: &mut PrettyEmitter, part: &KiCadLibPart) -> Result<(), SourceBundleError> {
    output.open("libpart")?;
    output.quoted_form("lib", &part.lib)?;
    output.quoted_form("part", &part.part)?;
    output.nonempty_form("description", &part.description)?;
    output.nonempty_form("docs", &part.docs)?;
    if !part.footprints_filter.is_empty() {
        output.open("footprints")?;
        for footprint in &part.footprints_filter {
            output.quoted_form("fp", footprint)?;
        }
        output.close()?;
    }
    if !part.fields.is_empty() {
        output.open("fields")?;
        for (name, value) in &part.fields {
            output.open("field")?;
            output.quoted_form("name", name)?;
            output.quoted(value)?;
            output.close()?;
        }
        output.close()?;
    }
    if !part.pins.is_empty() {
        output.open("pins")?;
        for pin in &part.pins {
            output.open("pin")?;
            output.quoted_form("num", &pin.number)?;
            output.quoted_form("name", &pin.name)?;
            output.quoted_form("type", &pin.pin_type)?;
            output.close()?;
        }
        output.close()?;
    }
    output.close()
}

fn emit_nets(
    output: &mut PrettyEmitter,
    nets: &[KiCadNet],
    included: &HashSet<&str>,
) -> Result<(), SourceBundleError> {
    output.open("nets")?;
    for net in nets {
        output.open("net")?;
        output.quoted_form("code", &net.code.to_string())?;
        output.quoted_form("name", &net.name)?;
        for terminal in &net.terminals {
            if !included.contains(terminal.designator.as_str()) {
                continue;
            }
            output.open("node")?;
            output.quoted_form("ref", &terminal.designator)?;
            output.quoted_form("pin", &terminal.pin)?;
            output.nonempty_form("pinfunction", &terminal.pin_name)?;
            output.nonempty_form("pintype", &terminal.pin_type)?;
            output.close()?;
        }
        output.close()?;
    }
    output.close()
}

fn nonempty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}

struct PrettyEmitter {
    text: String,
    depth: usize,
    maximum: usize,
}

impl PrettyEmitter {
    fn new(maximum: usize) -> Self {
        Self {
            text: String::new(),
            depth: 0,
            maximum,
        }
    }

    fn open(&mut self, head: &str) -> Result<(), SourceBundleError> {
        if !self.text.is_empty() {
            self.push("\n")?;
            self.indent(self.depth)?;
        }
        self.push("(")?;
        self.push(head)?;
        self.depth = self.depth.saturating_add(1);
        Ok(())
    }

    fn close(&mut self) -> Result<(), SourceBundleError> {
        self.depth = self.depth.checked_sub(1).ok_or_else(emit_error)?;
        self.push("\n")?;
        self.indent(self.depth)?;
        self.push(")")
    }

    fn quoted_form(&mut self, head: &str, value: &str) -> Result<(), SourceBundleError> {
        self.open(head)?;
        self.quoted(value)?;
        self.close()
    }

    fn optional_quoted_form(&mut self, head: &str, value: &str) -> Result<(), SourceBundleError> {
        self.open(head)?;
        if !value.is_empty() {
            self.quoted(value)?;
        }
        self.close()
    }

    fn nonempty_form(&mut self, head: &str, value: &str) -> Result<(), SourceBundleError> {
        if value.is_empty() {
            Ok(())
        } else {
            self.quoted_form(head, value)
        }
    }

    fn quoted(&mut self, value: &str) -> Result<(), SourceBundleError> {
        self.push(" \"")?;
        for character in value.chars() {
            match character {
                '\n' => self.push("\\n")?,
                '\r' => self.push("\\r")?,
                '\\' => self.push("\\\\")?,
                '"' => self.push("\\\"")?,
                _ => {
                    let mut encoded = [0_u8; 4];
                    self.push(character.encode_utf8(&mut encoded))?;
                }
            }
        }
        self.push("\"")
    }

    fn indent(&mut self, depth: usize) -> Result<(), SourceBundleError> {
        let count = depth.checked_mul(2).ok_or_else(emit_error)?;
        for _ in 0..count {
            self.push(" ")?;
        }
        Ok(())
    }

    fn push(&mut self, value: &str) -> Result<(), SourceBundleError> {
        if self
            .text
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(limit_error());
        }
        self.text.write_str(value).map_err(|_| emit_error())
    }

    fn finish(mut self) -> Result<String, SourceBundleError> {
        if self.depth != 0 {
            return Err(emit_error());
        }
        self.push("\n")?;
        Ok(self.text)
    }
}

fn limit_error() -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        None,
        "KiCad netlist output exceeds max_output_bytes",
    )
}

fn emit_error() -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Schematic,
        None,
        "KiCad netlist emitter state is invalid",
    )
}
