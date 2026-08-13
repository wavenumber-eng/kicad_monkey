use kicad_monkey_core::{ErrorKind, PcbFamily, PcbLimits, PcbSelection, PcbView};

const SOURCE: &str = r#"(kicad_pcb
  (footprint "Demo:Text"
    (layer "F.Cu")
    (fp_text reference "R1" (at 1 2 90) hide
      (layer "F.SilkS" knockout) (uuid text-id)
      (effects (font (face "Inter") (size 1.5 2.5) (thickness 0.2)
        (line_spacing 1.1) (bold yes) italic (color 1 2 3 0.5))
        (justify left top mirror) (href "https://example.invalid")))
    (fp_text user "cached" (at 0 0 0) (layer "B.SilkS")
      (effects (font (size 1 1)) (hide yes))
      (render_cache "cached" 0 (polygon (pts (xy 0 0) (xy 1 0)))))
    (fp_text_box "boxed" locked
      (pts (xy -2 -1) (xy 3 -1) (xy 3 4) (xy -2 4))
      (margins 1 2 3 4) (angle 45) (layer "F.Fab")
      (effects (font (size 2 3)))
      (border no) (stroke (width 0.12) (type dash)) (knockout) (uuid box-id)
      (render_cache "boxed" 45)))
)"#;

#[test]
fn footprint_text_preserves_authored_effects_and_cache_evidence() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board");
    let texts = view
        .footprint_texts()
        .collect::<Result<Vec<_>, _>>()
        .expect("texts");
    assert_eq!(texts.len(), 2);
    assert_text_identity(&texts[0]);
    assert_text_effects(&texts[0]);
    assert!(texts[0].render_cache_range.is_none());
    assert!(texts[1].hidden);
    let cache = texts[1].render_cache_range.clone().expect("cache range");
    assert!(SOURCE[cache].starts_with("(render_cache"));
}

fn assert_text_identity(text: &kicad_monkey_core::PcbFootprintText) {
    assert_eq!(
        (text.kind.as_str(), text.text.as_str()),
        ("reference", "R1")
    );
    assert_eq!((text.at.x, text.at.y, text.angle), (1.0, 2.0, 90.0));
    assert_eq!(text.layer, "F.SilkS");
    assert!(text.knockout);
    assert!(text.hidden);
    assert_eq!(text.uuid.as_deref(), Some("text-id"));
}

fn assert_text_effects(text: &kicad_monkey_core::PcbFootprintText) {
    assert_eq!(text.effects.font.face.as_deref(), Some("Inter"));
    assert_eq!(
        (text.effects.font.size_x, text.effects.font.size_y),
        (2.5, 1.5)
    );
    assert_eq!(text.effects.font.thickness, Some(0.2));
    assert_eq!(text.effects.font.line_spacing, Some(1.1));
    assert!(text.effects.font.bold);
    assert!(text.effects.font.italic);
    assert_eq!(text.effects.justify, ["left", "top", "mirror"]);
    assert_eq!(
        text.effects.href.as_deref(),
        Some("https://example.invalid")
    );
    let color = text.effects.font.color.expect("font color");
    assert_eq!(
        (color.red, color.green, color.blue, color.alpha),
        (1, 2, 3, 0.5)
    );
}

#[test]
fn footprint_text_box_preserves_polygon_defaults_and_optional_booleans() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board");
    let text_box = view
        .footprint_text_boxes()
        .next()
        .expect("text box")
        .expect("typed text box");
    assert_text_box_geometry(&text_box);
    assert_text_box_style(&text_box);
}

fn assert_text_box_geometry(text_box: &kicad_monkey_core::PcbFootprintTextBox) {
    assert_eq!(text_box.text, "boxed");
    assert_eq!((text_box.start.x, text_box.start.y), (-2.0, -1.0));
    assert_eq!((text_box.end.x, text_box.end.y), (3.0, 4.0));
    assert_eq!(text_box.margins, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(text_box.angle, 45.0);
    assert_eq!(text_box.polygon_points.len(), 4);
}

fn assert_text_box_style(text_box: &kicad_monkey_core::PcbFootprintTextBox) {
    assert_eq!(text_box.layer, "F.Fab");
    assert!(text_box.locked);
    assert_eq!(text_box.border, Some(false));
    assert_eq!(text_box.knockout, Some(true));
    assert_eq!(text_box.stroke_width, Some(0.12));
    assert_eq!(text_box.stroke_kind.as_deref(), Some("dash"));
    assert_eq!(text_box.uuid.as_deref(), Some("box-id"));
    assert_eq!(text_box.effects.as_ref().expect("effects").font.size_x, 3.0,);
    assert!(text_box.render_cache_range.is_some());
}

#[test]
fn text_families_are_selectable_and_independently_bounded() {
    let full = PcbView::parse(SOURCE, PcbLimits::default()).expect("full");
    let selected = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::FootprintTexts),
    )
    .expect("selected text");
    assert_eq!(
        selected
            .footprint_texts()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.footprint_texts()
            .collect::<Result<Vec<_>, _>>()
            .expect("full")
    );
    assert_eq!(selected.counts().footprint_texts, 2);
    assert_eq!(selected.counts().footprints, 0);
    assert_eq!(selected.footprint_text_boxes().count(), 0);

    for (selection, limits) in [
        (
            PcbFamily::FootprintTexts,
            PcbLimits {
                max_footprint_texts: 1,
                ..PcbLimits::default()
            },
        ),
        (
            PcbFamily::FootprintTextBoxes,
            PcbLimits {
                max_footprint_text_boxes: 0,
                ..PcbLimits::default()
            },
        ),
    ] {
        assert_eq!(
            PcbView::parse_selected(SOURCE, limits, PcbSelection::only(selection))
                .expect_err("family limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let view = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_text_box_points: 4,
            ..PcbLimits::default()
        },
    )
    .expect("exact point limit");
    assert_eq!(
        view.footprint_text_boxes()
            .next()
            .expect("text box")
            .expect("exact point limit")
            .polygon_points
            .len(),
        4
    );
    let view = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_text_box_points: 3,
            ..PcbLimits::default()
        },
    )
    .expect("lazy point rejection");
    assert_eq!(
        view.footprint_text_boxes()
            .next()
            .expect("text box")
            .expect_err("point limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn nested_text_metadata_limits_fail_at_the_requested_boundary() {
    for limits in [
        PcbLimits {
            max_text_effect_children: 2,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_text_font_children: 5,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_text_justify_tokens: 2,
            ..PcbLimits::default()
        },
    ] {
        let view = PcbView::parse(SOURCE, limits).expect("lazy nested metadata limit");
        assert_eq!(
            view.footprint_texts()
                .next()
                .expect("text")
                .expect_err("nested resource limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }
}
