//! Native worksheet semantic projection and exact-write gate.

use kicad_monkey_core::{
    WorksheetCorner, WorksheetDocument, WorksheetFormat, WorksheetItem, WorksheetLimits,
    WorksheetPoint, WorksheetRepeat,
};
use serde_json::{Value, json};
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for path in std::env::args_os().skip(1).map(PathBuf::from) {
        files.push(project(&path)?);
    }
    if files.is_empty() {
        return Err("no worksheet inputs supplied".into());
    }
    println!(
        "{}",
        serde_json::to_string(&json!({
            "schema": "kicad_monkey.worksheet_gate_evidence.a0",
            "file_count": files.len(),
            "files": files,
        }))?
    );
    Ok(())
}

fn project(path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let limits = WorksheetLimits::default();
    let file = std::fs::File::open(path)?;
    let document = WorksheetDocument::from_reader(BufReader::new(file), limits)?;
    let view = document.view()?;
    let metadata = view.metadata()?;
    let setup = view.setup()?;
    let items = view.items().collect::<Result<Vec<_>, _>>()?;
    let mut first_write = Vec::new();
    document.write_to(&mut first_write)?;
    if first_write != document.source().as_bytes() {
        return Err(format!("exact write changed {}", path.display()).into());
    }
    let reparsed = WorksheetDocument::from_reader(Cursor::new(&first_write), limits)?;
    let second = reparsed.view()?.items().collect::<Result<Vec<_>, _>>()?;
    if second != items {
        return Err(format!("worksheet semantics changed for {}", path.display()).into());
    }
    let mut second_write = Vec::new();
    reparsed.write_to(&mut second_write)?;
    if second_write != first_write {
        return Err(format!("stable write changed {}", path.display()).into());
    }
    Ok(json!({
        "path": path.to_string_lossy(),
        "source_bytes": first_write.len(),
        "format": match metadata.format {
            WorksheetFormat::Modern => "kicad_wks",
            WorksheetFormat::Legacy => "page_layout",
        },
        "version": metadata.version,
        "generator": metadata.generator,
        "generator_version": metadata.generator_version,
        "setup": {
            "text_size_x": setup.text_size_x,
            "text_size_y": setup.text_size_y,
            "linewidth": setup.line_width,
            "textlinewidth": setup.text_line_width,
            "left_margin": setup.left_margin,
            "right_margin": setup.right_margin,
            "top_margin": setup.top_margin,
            "bottom_margin": setup.bottom_margin,
        },
        "items": items.iter().map(item).collect::<Vec<_>>(),
        "exact_first_write": true,
        "stable_second_write": true,
    }))
}

fn item(value: &WorksheetItem) -> Value {
    match value {
        WorksheetItem::Line(value) => json!({
            "kind": "line", "name": value.name, "comment": value.comment,
            "option": value.option, "start": point(value.start), "end": point(value.end),
            "linewidth": value.line_width, "repeat": repeat(value.repeat),
        }),
        WorksheetItem::Rect(value) => json!({
            "kind": "rect", "name": value.name, "comment": value.comment,
            "option": value.option, "start": point(value.start), "end": point(value.end),
            "linewidth": value.line_width, "repeat": repeat(value.repeat),
        }),
        WorksheetItem::Polygon(value) => json!({
            "kind": "polygon", "name": value.name, "comment": value.comment,
            "option": value.option, "position": point(value.position), "rotate": value.rotate,
            "linewidth": value.line_width, "repeat": repeat(value.repeat),
            "point_sets": value.point_sets,
        }),
        WorksheetItem::Text(value) => json!({
            "kind": "tbtext", "text": value.text, "name": value.name,
            "comment": value.comment, "option": value.option,
            "position": point(value.position), "rotate": value.rotate,
            "repeat": repeat(value.repeat), "justify": value.justify,
            "max_length": value.max_length, "max_height": value.max_height,
            "font": {
                "size_x": value.font.size_x, "size_y": value.font.size_y,
                "linewidth": value.font.line_width, "bold": value.font.bold,
                "italic": value.font.italic, "face": value.font.face,
                "color": value.font.color.map(|color| json!([
                    color.red, color.green, color.blue, color.alpha
                ])),
            },
        }),
        WorksheetItem::Bitmap(value) => json!({
            "kind": "bitmap", "name": value.name, "comment": value.comment,
            "option": value.option, "position": point(value.position), "scale": value.scale,
            "repeat": repeat(value.repeat), "data_parts": value.data_parts,
        }),
    }
}

fn point(value: WorksheetPoint) -> Value {
    json!({
        "x": value.x,
        "y": value.y,
        "corner": match value.corner {
            WorksheetCorner::None => "",
            WorksheetCorner::LeftTop => "ltcorner",
            WorksheetCorner::RightTop => "rtcorner",
            WorksheetCorner::LeftBottom => "lbcorner",
            WorksheetCorner::RightBottom => "rbcorner",
        },
    })
}

fn repeat(value: WorksheetRepeat) -> Value {
    json!({
        "count": value.count,
        "increment_x": value.increment_x,
        "increment_y": value.increment_y,
        "increment_label": value.increment_label,
    })
}
