//! Transactional public native Toon command.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use kicad_monkey_core::{PcbLimits, PcbView};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::pcb_svg::geometer::{
    GEOMETER_C_ABI_GENERATION, GEOMETER_RELEASE, GEOMETER_SOURCE_REVISION, NativeGeometer,
};
use crate::pcb_svg::preview::{
    BoardSide, ToonIllustrationFailure, ToonIllustrationJob, ToonModelCache, ToonModelIllustrator,
    ToonPresentation, render_footprint_toon_preview_with_cache_and_layers,
    render_toon_preview_with_cache_and_layers,
};
use crate::toon_config::{
    ResolvedToonView, TOON_CONFIG_FILENAME, resolve_toon_config, resolved_views, write_config,
};
use crate::{ToonOptions, ToonSides};

const MAX_BOARD_BYTES: u64 = 256 * 1024 * 1024;
static TRANSACTION_ID: AtomicU64 = AtomicU64::new(0);

struct ToonRenderView {
    presentation: ToonPresentation,
    layers: Vec<String>,
}

struct ToonRenderViews {
    top: ToonRenderView,
    bottom: ToonRenderView,
}

struct ToonVariant {
    name: Option<String>,
    folder: String,
    excluded_components: BTreeSet<String>,
}

struct RenderedVariant {
    name: Option<String>,
    folder: String,
    sides: Vec<(BoardSide, crate::pcb_svg::preview::ToonPreview)>,
}

struct ResolvedToonInput {
    board: PathBuf,
    project: Option<PathBuf>,
}

impl ToonRenderViews {
    fn for_side(&self, side: BoardSide) -> &ToonRenderView {
        match side {
            BoardSide::Top => &self.top,
            BoardSide::Bottom => &self.bottom,
        }
    }
}

fn render_view(
    resolved: ResolvedToonView,
    assembly: bool,
    side: BoardSide,
) -> Result<ToonRenderView, ToonError> {
    let presentation = ToonPresentation::new_configured(
        resolved.colors.soldermask,
        resolved.colors.silkscreen,
        resolved.style,
    )
    .map_err(|error| ToonError::context("invalid Toon presentation", error))?;
    let mut layers = resolved.layers;
    if assembly {
        let token = format!(
            "ASSEMBLY_DESIGNATORS_{}",
            side_name(side).to_ascii_uppercase()
        );
        if !layers.iter().any(|layer| layer == &token) {
            let illustration = format!("ILLUSTRATION_{}", side_name(side).to_ascii_uppercase());
            let index = layers
                .iter()
                .position(|layer| layer == &illustration)
                .map_or(layers.len(), |index| index + 1);
            layers.insert(index, token);
        }
    }
    Ok(ToonRenderView {
        presentation,
        layers,
    })
}

struct LazyNativeGeometer {
    inner: RefCell<Option<Vec<NativeGeometer>>>,
    requested_workers: usize,
    started_workers: Cell<usize>,
    startup_ms: Cell<f64>,
}

impl LazyNativeGeometer {
    fn new(requested_workers: usize) -> Self {
        Self {
            inner: RefCell::new(None),
            requested_workers,
            started_workers: Cell::new(0),
            startup_ms: Cell::new(0.0),
        }
    }

    fn startup_ms(&self) -> f64 {
        self.startup_ms.get()
    }

    fn started_workers(&self) -> usize {
        self.started_workers.get()
    }

    fn close(self) -> Result<(), crate::pcb_svg::geometer::NativeGeometerError> {
        match self.inner.into_inner() {
            Some(geometers) => {
                for geometer in geometers {
                    geometer.close()?;
                }
                Ok(())
            }
            None => Ok(()),
        }
    }
}

impl ToonModelIllustrator for LazyNativeGeometer {
    fn illustrate_models(
        &self,
        jobs: Vec<ToonIllustrationJob>,
    ) -> Result<
        Vec<Result<geometer_client::ModelIllustrationGeometry, String>>,
        ToonIllustrationFailure,
    > {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        let existing_workers = self
            .inner
            .borrow()
            .as_ref()
            .map_or(0, |workers| workers.len());
        let desired_workers = self.requested_workers.min(jobs.len());
        if desired_workers > existing_workers {
            let started = Instant::now();
            let worker_count = desired_workers - existing_workers;
            let connected = std::thread::scope(|scope| {
                let handles = (0..worker_count)
                    .map(|_| scope.spawn(NativeGeometer::connect_discovered))
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .map(|handle| {
                        handle
                            .join()
                            .map_err(|_| "Geometer worker panicked while starting".to_owned())?
                            .map_err(|error| error.to_string())
                    })
                    .collect::<Result<Vec<_>, String>>()
            });
            self.startup_ms
                .set(self.startup_ms.get() + milliseconds(started.elapsed()));
            let connected = connected.map_err(ToonIllustrationFailure::Startup)?;
            let mut inner = self.inner.borrow_mut();
            inner.get_or_insert_with(Vec::new).extend(connected);
            self.started_workers
                .set(inner.as_ref().map_or(0, |workers| workers.len()));
        }
        let mut inner = self.inner.borrow_mut();
        let geometers = inner.as_mut().expect("lazy Geometer is connected");
        let next = AtomicUsize::new(0);
        let batches = std::thread::scope(|scope| {
            let handles = geometers
                .iter_mut()
                .map(|geometer| {
                    let jobs = &jobs;
                    let next = &next;
                    scope.spawn(move || {
                        let mut completed = Vec::new();
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(job) = jobs.get(index) else {
                                break;
                            };
                            completed.push((
                                index,
                                geometer
                                    .illustrate_model(job.request.clone(), job.step.to_vec())
                                    .map_err(|error| error.to_string()),
                            ));
                        }
                        completed
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| {
                    handle.join().map_err(|_| {
                        ToonIllustrationFailure::Startup(
                            "Geometer worker panicked during illustration".to_owned(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        let mut outcomes = std::iter::repeat_with(|| None)
            .take(jobs.len())
            .collect::<Vec<_>>();
        for (index, outcome) in batches.into_iter().flatten() {
            outcomes[index] = Some(outcome);
        }
        outcomes
            .into_iter()
            .map(|outcome| {
                outcome.ok_or_else(|| {
                    ToonIllustrationFailure::Startup(
                        "Geometer worker omitted an illustration result".to_owned(),
                    )
                })
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct ToonRun {
    pub output_dir: PathBuf,
    pub artifact_count: usize,
    pub rendered_model_count: usize,
    pub geometer_request_count: usize,
    pub model_cache_hit_count: usize,
    pub persistent_model_cache_hit_count: usize,
    pub warning_count: usize,
    pub timings_path: Option<PathBuf>,
    pub wrote_config: bool,
}

#[derive(Debug)]
pub struct ToonError(String);

impl ToonError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    fn context(message: &str, error: impl fmt::Display) -> Self {
        Self(format!("{message}: {error}"))
    }
}

impl fmt::Display for ToonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ToonError {}

#[allow(
    clippy::too_many_lines,
    reason = "the public command keeps validation, render shutdown, and transactional publication visibly ordered"
)]
pub fn run_toon(options: &ToonOptions) -> Result<ToonRun, ToonError> {
    let total_started = Instant::now();
    if let Some(target) = options.write_config.as_deref() {
        let source_config = options.config.as_deref().map(absolute_path).transpose()?;
        let resolved = resolve_toon_config(
            source_config.as_deref(),
            options.theme,
            options.soldermask_color.as_deref(),
        )
        .map_err(|error| ToonError::new(format!("could not resolve Toon config: {error}")))?;
        let target = absolute_path(target)?;
        write_config(&target, &resolved)
            .map_err(|error| ToonError::new(format!("could not write Toon config: {error}")))?;
        return Ok(ToonRun {
            output_dir: target,
            artifact_count: 0,
            rendered_model_count: 0,
            geometer_request_count: 0,
            model_cache_hit_count: 0,
            persistent_model_cache_hit_count: 0,
            warning_count: 0,
            timings_path: None,
            wrote_config: true,
        });
    }
    let input_started = Instant::now();
    let resolved_input = resolve_input(options.input.as_deref(), options.pcbdoc.as_deref())?;
    let input = resolved_input.board;
    let source_bytes = read_bounded(&input)?;
    let source = std::str::from_utf8(&source_bytes)
        .map_err(|error| ToonError::context("PCB source is not UTF-8", error))?;
    let board_name = input
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ToonError::new("PCB filename has no valid Unicode stem"))?;
    let input_ms = milliseconds(input_started.elapsed());
    let destination = options
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from("output/toon"));
    let destination = absolute_path(&destination)?;
    let timings_path = options.timings.as_deref().map(absolute_path).transpose()?;
    let config_path = options
        .config
        .as_deref()
        .map(absolute_path)
        .transpose()?
        .unwrap_or_else(|| {
            input
                .parent()
                .unwrap_or(Path::new("."))
                .join(TOON_CONFIG_FILENAME)
        });
    let config_existed = config_path.exists();
    let config = resolve_toon_config(
        Some(&config_path),
        options.theme,
        options.soldermask_color.as_deref(),
    )
    .map_err(|error| ToonError::new(format!("could not resolve Toon config: {error}")))?;
    if !config_existed {
        write_config(&config_path, &config)
            .map_err(|error| ToonError::new(format!("could not create Toon config: {error}")))?;
    }
    let resolved_views = resolved_views(&config)
        .map_err(|error| ToonError::new(format!("could not resolve Toon views: {error}")))?;
    let config_sha256 =
        hex_digest(&serde_json::to_vec(&config).map_err(|error| {
            ToonError::context("could not serialize resolved Toon config", error)
        })?);
    let (persistent_cache_dir, cache_mode) = if options.no_cache {
        (None, "disabled")
    } else if let Some(path) = options.cache_dir.as_deref() {
        (Some(absolute_path(path)?), "explicit")
    } else {
        match default_toon_cache_dir() {
            Some(path) => (Some(path), "default"),
            None => (None, "unavailable"),
        }
    };
    if destination.exists() && !destination.is_dir() {
        return Err(ToonError::new(format!(
            "Toon output exists and is not a directory: {}",
            destination.display()
        )));
    }
    let presentation_name = if options.soldermask_color.is_some() {
        "custom"
    } else if let Some(theme) = options.theme {
        theme.name()
    } else {
        "config"
    };
    let presentations = ToonRenderViews {
        top: render_view(resolved_views.top, options.assembly, BoardSide::Top)?,
        bottom: render_view(resolved_views.bottom, options.assembly, BoardSide::Bottom)?,
    };
    let variants = resolve_variants(resolved_input.project.as_deref(), options)?;
    let variant_scoped = options.variant.is_some() || options.all_variants;
    let geometer = LazyNativeGeometer::new(options.workers);

    let render_started = Instant::now();
    let mut model_cache = ToonModelCache::with_persistent_dir(persistent_cache_dir);
    let result = variants
        .into_iter()
        .map(|variant| {
            render_requested(
                source,
                board_name,
                options.sides,
                &presentations,
                &geometer,
                &mut model_cache,
                options.footprint.as_deref(),
                Some(&variant.excluded_components),
            )
            .map(|sides| RenderedVariant {
                name: variant.name,
                folder: variant.folder,
                sides,
            })
        })
        .collect::<Result<Vec<_>, ToonError>>();
    let geometer_start_ms = geometer.startup_ms();
    let started_workers = geometer.started_workers();
    let close = geometer.close();
    let rendered = result?;
    close.map_err(|error| ToonError::context("could not close Geometer", error))?;
    let render_elapsed_ms = milliseconds(render_started.elapsed());
    let render_ms = (render_elapsed_ms - geometer_start_ms).max(0.0);

    let publication_started = Instant::now();
    let parent = destination
        .parent()
        .ok_or_else(|| ToonError::new("Toon output directory has no parent"))?;
    fs::create_dir_all(parent)
        .map_err(|error| ToonError::context("could not create Toon output parent", error))?;
    let transaction = create_transaction(parent)?;
    let staging = transaction.join("output");
    fs::create_dir(&staging)
        .map_err(|error| ToonError::context("could not create Toon staging directory", error))?;
    let staged = stage_artifacts(
        &staging,
        &input,
        board_name,
        options.sides,
        &presentations,
        presentation_name,
        &config_path,
        &config_sha256,
        options.footprint.as_deref(),
        options.assembly,
        variant_selection_json(options),
        variant_scoped,
        &rendered,
    );
    if let Err(error) = staged {
        let _cleanup = fs::remove_dir_all(&transaction);
        return Err(error);
    }
    publish(&staging, &destination, &transaction)?;
    let _cleanup = fs::remove_dir_all(&transaction);
    let publication_ms = milliseconds(publication_started.elapsed());

    if let Some(path) = &timings_path {
        write_timings(
            path,
            &input,
            options.sides,
            &rendered,
            cache_mode,
            presentation_name,
            &presentations,
            &config_path,
            &config_sha256,
            options.footprint.as_deref(),
            options.assembly,
            variant_selection_json(options),
            options.workers,
            started_workers,
            input_ms,
            geometer_start_ms,
            render_ms,
            publication_ms,
            milliseconds(total_started.elapsed()),
        )?;
    }

    Ok(ToonRun {
        output_dir: destination,
        artifact_count: rendered.iter().map(|variant| variant.sides.len()).sum(),
        rendered_model_count: rendered
            .iter()
            .flat_map(|variant| &variant.sides)
            .map(|(_, preview)| preview.rendered_model_count)
            .sum(),
        geometer_request_count: rendered
            .iter()
            .flat_map(|variant| &variant.sides)
            .map(|(_, preview)| preview.geometer_request_count)
            .sum(),
        model_cache_hit_count: rendered
            .iter()
            .flat_map(|variant| &variant.sides)
            .map(|(_, preview)| preview.model_cache_hit_count)
            .sum(),
        persistent_model_cache_hit_count: rendered
            .iter()
            .flat_map(|variant| &variant.sides)
            .map(|(_, preview)| preview.persistent_model_cache_hit_count)
            .sum(),
        warning_count: rendered
            .iter()
            .flat_map(|variant| &variant.sides)
            .map(|(_, preview)| preview.warnings.len())
            .sum(),
        timings_path,
        wrote_config: false,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "the timing contract records the complete command phase and worker context"
)]
fn write_timings(
    path: &Path,
    input: &Path,
    sides: ToonSides,
    rendered: &[RenderedVariant],
    cache_mode: &str,
    presentation_name: &str,
    presentations: &ToonRenderViews,
    config_path: &Path,
    config_sha256: &str,
    footprint_reference: Option<&str>,
    assembly_designators: bool,
    variant_selection: serde_json::Value,
    requested_workers: usize,
    started_workers: usize,
    input_ms: f64,
    geometer_start_ms: f64,
    render_ms: f64,
    publication_ms: f64,
    total_ms: f64,
) -> Result<(), ToonError> {
    let side_timings = rendered
        .iter()
        .flat_map(|variant| {
            variant.sides.iter().map(move |(side, preview)| {
                let svg = svg_for_variant(&preview.svg, variant.name.as_deref());
                json!({
                    "variant": variant.name.as_deref().unwrap_or("base"),
                    "side": side_name(*side),
                    "model_instances": preview.rendered_model_count,
                    "geometer_requests": preview.geometer_request_count,
                    "model_cache_hits": preview.model_cache_hit_count,
                    "persistent_model_cache_hits": preview.persistent_model_cache_hit_count,
                    "warning_count": preview.warnings.len(),
                    "svg_bytes": svg.len(),
                    "svg_sha256": hex_digest(svg.as_bytes()),
                    "phases_ms": {
                        "preparation": preview.timings.preparation_ms,
                        "physical_render": preview.timings.physical_render_ms,
                        "model_illustration": preview.timings.model_illustration_ms,
                        "composition": preview.timings.composition_ms,
                        "total": preview.timings.total_ms,
                    },
                })
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema": "kicad_cruncher.toon_timings.a0",
        "source": input
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ToonError::new("PCB filename is not valid Unicode"))?,
        "requested_sides": sides.name(),
        "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "cruncher_version": env!("CARGO_PKG_VERSION"),
        "cache_mode": cache_mode,
        "selection": selection_json(footprint_reference),
        "variant_selection": variant_selection,
        "presentation": presentation_json(
            presentation_name,
            presentations,
            assembly_designators,
        ),
        "config": {
            "file": config_filename(config_path)?,
            "resolved_sha256": config_sha256,
        },
        "workers": {
            "requested": requested_workers,
            "started": started_workers,
        },
        "geometer": {
            "release": GEOMETER_RELEASE,
            "c_abi_generation": GEOMETER_C_ABI_GENERATION,
            "source_revision": GEOMETER_SOURCE_REVISION,
        },
        "command_phases_ms": {
            "input": input_ms,
            "geometer_start": geometer_start_ms,
            "render_and_shutdown": render_ms,
            "publication": publication_ms,
            "total_before_timings_write": total_ms,
        },
        "sides": side_timings,
    });
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| ToonError::context("could not create Toon timings parent", error))?;
    }
    let payload = serde_json::to_vec_pretty(&payload)
        .map_err(|error| ToonError::context("could not serialize Toon timings", error))?;
    fs::write(path, payload)
        .map_err(|error| ToonError::context("could not write Toon timings", error))
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

#[allow(
    clippy::too_many_arguments,
    reason = "variant rendering passes the shared presentation, worker, cache, selection, and population contexts explicitly"
)]
fn render_requested<I: ToonModelIllustrator + ?Sized>(
    source: &str,
    board_name: &str,
    sides: ToonSides,
    presentations: &ToonRenderViews,
    geometer: &I,
    model_cache: &mut ToonModelCache,
    footprint_reference: Option<&str>,
    excluded_components: Option<&BTreeSet<String>>,
) -> Result<Vec<(BoardSide, crate::pcb_svg::preview::ToonPreview)>, ToonError> {
    let mut rendered = Vec::new();
    let render_sides = match footprint_reference {
        Some(reference) => {
            let side = footprint_side(source, reference)?;
            if !sides.board_sides().contains(&side) {
                return Err(ToonError::new(format!(
                    "footprint {reference:?} is on the {} side, outside requested --side {}",
                    side_name(side),
                    sides.name(),
                )));
            }
            vec![side]
        }
        None => sides.board_sides(),
    };
    for side in render_sides {
        let view = presentations.for_side(side);
        let document_id = format!("{board_name}-{}", side_name(side));
        let preview = match footprint_reference {
            Some(reference) => render_footprint_toon_preview_with_cache_and_layers(
                source,
                document_id,
                side,
                reference,
                geometer,
                &view.presentation,
                model_cache,
                &view.layers,
                excluded_components,
            ),
            None => render_toon_preview_with_cache_and_layers(
                source,
                document_id,
                side,
                geometer,
                &view.presentation,
                model_cache,
                &view.layers,
                excluded_components,
            ),
        }
        .map_err(|error| ToonError::context("could not render native Toon SVG", error))?;
        rendered.push((side, preview));
    }
    Ok(rendered)
}

#[allow(
    clippy::too_many_arguments,
    reason = "artifact staging binds render results to their full portable command provenance"
)]
fn stage_artifacts(
    staging: &Path,
    input: &Path,
    board_name: &str,
    sides: ToonSides,
    presentations: &ToonRenderViews,
    presentation_name: &str,
    config_path: &Path,
    config_sha256: &str,
    footprint_reference: Option<&str>,
    assembly_designators: bool,
    variant_selection: serde_json::Value,
    variant_scoped: bool,
    rendered: &[RenderedVariant],
) -> Result<(), ToonError> {
    let mut artifacts = Vec::new();
    for variant in rendered {
        let output_root = if variant_scoped {
            let directory = staging.join(&variant.folder);
            fs::create_dir(&directory).map_err(|error| {
                ToonError::context("could not create staged variant directory", error)
            })?;
            directory
        } else {
            staging.to_path_buf()
        };
        for (side, preview) in &variant.sides {
            let filename = match footprint_reference {
                Some(reference) => format!(
                    "{board_name}__{}__toon_{}.svg",
                    filename_token(reference),
                    side_name(*side)
                ),
                None => format!("{board_name}__toon_{}.svg", side_name(*side)),
            };
            let svg = svg_for_variant(&preview.svg, variant.name.as_deref());
            fs::write(output_root.join(&filename), svg.as_bytes())
                .map_err(|error| ToonError::context("could not write staged Toon SVG", error))?;
            let relative_filename = if variant_scoped {
                format!("{}/{}", variant.folder, filename)
            } else {
                filename
            };
            artifacts.push(json!({
                "file": relative_filename,
                "variant": variant.name.as_deref().unwrap_or("base"),
                "side": side_name(*side),
                "svg_bytes": svg.len(),
                "svg_sha256": hex_digest(svg.as_bytes()),
                "soldermask_color": preview.soldermask_color,
                "silkscreen_color": preview.silkscreen_color,
                "rendered_model_count": preview.rendered_model_count,
                "geometer_request_count": preview.geometer_request_count,
                "model_cache_hit_count": preview.model_cache_hit_count,
                "persistent_model_cache_hit_count": preview.persistent_model_cache_hit_count,
                "warnings": preview.warnings,
            }));
        }
    }
    let manifest = json!({
        "schema": "kicad_cruncher.toon_manifest.a0",
        "version": "a0",
        "source": input
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ToonError::new("PCB filename is not valid Unicode"))?,
        "requested_sides": sides.name(),
        "selection": selection_json(footprint_reference),
        "variant_selection": variant_selection,
        "presentation": presentation_json(
            presentation_name,
            presentations,
            assembly_designators,
        ),
        "config": {
            "file": config_filename(config_path)?,
            "resolved_sha256": config_sha256,
        },
        "geometer": {
            "release": GEOMETER_RELEASE,
            "c_abi_generation": GEOMETER_C_ABI_GENERATION,
            "source_revision": GEOMETER_SOURCE_REVISION,
        },
        "artifacts": artifacts,
    });
    let manifest = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| ToonError::context("could not serialize Toon manifest", error))?;
    fs::write(staging.join("manifest.json"), manifest)
        .map_err(|error| ToonError::context("could not write Toon manifest", error))
}

fn svg_for_variant(svg: &str, variant: Option<&str>) -> String {
    let Some(variant) = variant else {
        return svg.to_owned();
    };
    svg.replacen(
        "<svg ",
        &format!("<svg data-variant=\"{}\" ", escape_manifest_xml(variant)),
        1,
    )
}

fn escape_manifest_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn presentation_json(
    presentation_name: &str,
    presentations: &ToonRenderViews,
    assembly_designators: bool,
) -> serde_json::Value {
    json!({
        "theme": presentation_name,
        "assembly_designators": assembly_designators,
        "sides": {
            "top": {
                "soldermask_color": presentations.top.presentation.requested_soldermask_color(),
                "silkscreen_color": presentations.top.presentation.requested_silkscreen_color(),
            },
            "bottom": {
                "soldermask_color": presentations.bottom.presentation.requested_soldermask_color(),
                "silkscreen_color": presentations.bottom.presentation.requested_silkscreen_color(),
            },
        },
    })
}

fn config_filename(path: &Path) -> Result<&str, ToonError> {
    path.file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ToonError::new("Toon config filename is not valid Unicode"))
}

fn footprint_side(source: &str, reference: &str) -> Result<BoardSide, ToonError> {
    let view = PcbView::parse(source, PcbLimits::default())
        .map_err(|error| ToonError::context("could not parse PCB footprint selection", error))?;
    let matches = view
        .footprints()
        .filter_map(|footprint| match footprint {
            Ok(footprint) if footprint.reference.as_deref() == Some(reference) => {
                Some(Ok(footprint))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| ToonError::context("could not read PCB footprints", error))?;
    let footprint = match matches.as_slice() {
        [footprint] => footprint,
        [] => {
            return Err(ToonError::new(format!(
                "footprint reference {reference:?} was not found"
            )));
        }
        _ => {
            return Err(ToonError::new(format!(
                "footprint reference {reference:?} is not unique"
            )));
        }
    };
    Ok(
        if footprint
            .layer
            .as_deref()
            .is_some_and(|layer| layer.starts_with("B."))
        {
            BoardSide::Bottom
        } else {
            BoardSide::Top
        },
    )
}

fn selection_json(reference: Option<&str>) -> serde_json::Value {
    match reference {
        Some(reference) => json!({"kind": "footprint", "reference": reference}),
        None => json!({"kind": "board"}),
    }
}

fn filename_token(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn variant_selection_json(options: &ToonOptions) -> serde_json::Value {
    if options.all_variants {
        json!({"mode": "all"})
    } else if let Some(name) = options.variant.as_deref() {
        json!({"mode": "named", "name": name})
    } else {
        json!({"mode": "base"})
    }
}

fn resolve_variants(
    project: Option<&Path>,
    options: &ToonOptions,
) -> Result<Vec<ToonVariant>, ToonError> {
    if options.variant.is_none() && !options.all_variants {
        return Ok(vec![ToonVariant {
            name: None,
            folder: "base".to_owned(),
            excluded_components: BTreeSet::new(),
        }]);
    }
    let project = project.ok_or_else(|| {
        ToonError::new("--variant/--all-variants requires an adjacent .kicad_pro project")
    })?;
    let loaded = crate::design::load_design_sources(project)
        .map_err(|error| ToonError::context("could not load KiCad variant sources", error))?;
    let facts = crate::design::build_structured_design_facts_with_options(&loaded, false)
        .map_err(|error| ToonError::context("could not resolve KiCad variants", error))?;
    let rows = facts
        .design_json
        .get("variants")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ToonError::new("KiCad design variant catalog is unavailable"))?;
    let mut available = Vec::with_capacity(rows.len());
    for row in rows {
        let name = row
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| ToonError::new("KiCad project contains a variant without a name"))?;
        let excluded_components = row
            .get("dnp")
            .and_then(serde_json::Value::as_array)
            .map_or_else(BTreeSet::new, |values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            });
        available.push(ToonVariant {
            name: Some(name.to_owned()),
            folder: variant_folder(name)?,
            excluded_components,
        });
    }
    let mut selected = if options.all_variants {
        let mut selected = vec![ToonVariant {
            name: None,
            folder: "base".to_owned(),
            excluded_components: BTreeSet::new(),
        }];
        selected.extend(available);
        selected
    } else {
        let requested = options.variant.as_deref().expect("named variant requested");
        vec![
            available
                .into_iter()
                .find(|variant| variant.name.as_deref() == Some(requested))
                .ok_or_else(|| {
                    let names = rows
                        .iter()
                        .filter_map(|row| row.get("name")?.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    ToonError::new(format!(
                        "unknown variant {requested:?}; available: {}",
                        if names.is_empty() { "none" } else { &names }
                    ))
                })?,
        ]
    };
    let mut folders = BTreeSet::new();
    for variant in &selected {
        if !folders.insert(variant.folder.to_ascii_lowercase()) {
            return Err(ToonError::new(
                "selected variant names collide as portable output directories",
            ));
        }
    }
    selected.shrink_to_fit();
    Ok(selected)
}

fn variant_folder(name: &str) -> Result<String, ToonError> {
    let folder = filename_token(name).trim_matches('.').to_owned();
    if folder.is_empty() || folder == "." || folder == ".." {
        return Err(ToonError::new(format!(
            "variant name {name:?} cannot form a portable output directory"
        )));
    }
    Ok(folder)
}

fn resolve_input(
    input: Option<&Path>,
    pcbdoc: Option<&str>,
) -> Result<ResolvedToonInput, ToonError> {
    let candidate = match input {
        Some(input) => input.to_path_buf(),
        None => discover_input()?,
    };
    let candidate = candidate
        .canonicalize()
        .map_err(|error| ToonError::context("could not resolve Toon input", error))?;
    match candidate.extension().and_then(|value| value.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("kicad_pcb") => {
            if let Some(selector) = pcbdoc {
                let filename = candidate.file_name().and_then(|value| value.to_str());
                let stem = candidate.file_stem().and_then(|value| value.to_str());
                if filename != Some(selector) && stem != Some(selector) {
                    return Err(ToonError::new(format!(
                        "--doc {selector:?} does not select input board {}",
                        candidate.display()
                    )));
                }
            }
            let adjacent_project = candidate.with_extension("kicad_pro");
            Ok(ResolvedToonInput {
                board: candidate,
                project: adjacent_project.is_file().then_some(adjacent_project),
            })
        }
        Some(extension) if extension.eq_ignore_ascii_case("kicad_pro") => {
            let board = if let Some(selector) = pcbdoc {
                let mut board = candidate
                    .parent()
                    .ok_or_else(|| ToonError::new("project input has no parent directory"))?
                    .join(selector);
                if board.extension().is_none() {
                    board.set_extension("kicad_pcb");
                }
                board
            } else {
                candidate.with_extension("kicad_pcb")
            };
            if board.is_file() {
                let board = board.canonicalize().map_err(|error| {
                    ToonError::context("could not resolve selected project board", error)
                })?;
                if board.parent() != candidate.parent() {
                    return Err(ToonError::new(
                        "--doc must select a board adjacent to the KiCad project",
                    ));
                }
                Ok(ResolvedToonInput {
                    board,
                    project: Some(candidate),
                })
            } else {
                Err(ToonError::new(format!(
                    "project has no adjacent PCB: {}",
                    board.display()
                )))
            }
        }
        _ => Err(ToonError::new(
            "Toon input must be a .kicad_pcb or .kicad_pro file",
        )),
    }
}

fn discover_input() -> Result<PathBuf, ToonError> {
    let mut projects = discover_suffix("kicad_pro")?;
    if projects.len() == 1 {
        return Ok(projects.remove(0));
    }
    if projects.len() > 1 {
        return Err(ToonError::new(
            "multiple .kicad_pro files were found; supply the intended input explicitly",
        ));
    }
    let boards = discover_suffix("kicad_pcb")?;
    match boards.as_slice() {
        [board] => Ok(board.clone()),
        [] => Err(ToonError::new(
            "no .kicad_pro/.kicad_pcb input was supplied or found in the current directory",
        )),
        _ => Err(ToonError::new(
            "multiple .kicad_pcb files were found; supply the intended input explicitly",
        )),
    }
}

fn discover_suffix(extension: &str) -> Result<Vec<PathBuf>, ToonError> {
    let mut boards = fs::read_dir(".")
        .map_err(|error| ToonError::context("could not inspect current directory", error))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
        .collect::<Vec<_>>();
    boards.sort();
    Ok(boards)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ToonError> {
    let metadata = fs::metadata(path)
        .map_err(|error| ToonError::context("could not inspect PCB source", error))?;
    if !metadata.is_file() {
        return Err(ToonError::new("Toon input is not a regular file"));
    }
    if metadata.len() > MAX_BOARD_BYTES {
        return Err(ToonError::new(format!(
            "PCB source exceeds the {} byte Toon limit",
            MAX_BOARD_BYTES
        )));
    }
    fs::read(path).map_err(|error| ToonError::context("could not read PCB source", error))
}

fn absolute_path(path: &Path) -> Result<PathBuf, ToonError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| ToonError::context("could not resolve current directory", error))
    }
}

fn default_toon_cache_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|root| root.join("kicad-cruncher").join("toon-models-a0"));
    }
    if cfg!(target_os = "macos") {
        return std::env::var_os("HOME").map(PathBuf::from).map(|root| {
            root.join("Library")
                .join("Caches")
                .join("kicad-cruncher")
                .join("toon-models-a0")
        });
    }
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|root| PathBuf::from(root).join(".cache")))
        .map(|root| root.join("kicad-cruncher").join("toon-models-a0"))
}

fn create_transaction(parent: &Path) -> Result<PathBuf, ToonError> {
    for _ in 0..100 {
        let id = TRANSACTION_ID.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".kicad-cruncher-toon-{}-{id}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(ToonError::context(
                    "could not create Toon transaction directory",
                    error,
                ));
            }
        }
    }
    Err(ToonError::new(
        "could not allocate a unique Toon transaction directory",
    ))
}

fn publish(staging: &Path, destination: &Path, transaction: &Path) -> Result<(), ToonError> {
    let backup = transaction.join("previous");
    let had_previous = destination.exists();
    if had_previous && let Err(error) = fs::rename(destination, &backup) {
        let _cleanup = fs::remove_dir_all(transaction);
        return Err(ToonError::context(
            "could not stage previous Toon output",
            error,
        ));
    }
    if let Err(error) = fs::rename(staging, destination) {
        if had_previous && let Err(restore) = fs::rename(&backup, destination) {
            return Err(ToonError::new(format!(
                "could not publish Toon output ({error}); previous output restore also failed ({restore}); recovery data remains at {}",
                transaction.display()
            )));
        }
        let _cleanup = fs::remove_dir_all(transaction);
        return Err(ToonError::context("could not publish Toon output", error));
    }
    // Publication is complete at this point. Cleanup is best effort so a locked
    // obsolete backup cannot turn a successfully published artifact into a
    // reported transaction failure.
    if had_previous {
        let _cleanup = fs::remove_dir_all(&backup);
    }
    Ok(())
}

const fn side_name(side: BoardSide) -> &'static str {
    match side {
        BoardSide::Top => "top",
        BoardSide::Bottom => "bottom",
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut result = String::with_capacity(64);
    for byte in digest {
        use fmt::Write;
        write!(result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const SIMPLE_BOARD: &str = r#"(kicad_pcb
      (version 20250830) (generator pcbnew)
      (layers
        (0 "F.Cu" signal) (31 "B.Cu" signal)
        (36 "B.SilkS" user "Back Silkscreen")
        (37 "F.SilkS" user "Front Silkscreen")
        (44 "Edge.Cuts" user))
      (gr_rect (start 0 0) (end 10 5)
        (stroke (width 0.1) (type solid)) (fill none)
        (layer "Edge.Cuts") (uuid "edge"))
      (footprint "Test:J1" (layer "F.Cu") (at 5 2.5)
        (property "Reference" "J1" (at 0 0) (layer "F.SilkS"))
        (pad "1" thru_hole circle (at -1 0) (size 1 1) (drill 0.5)
          (layers "*.Cu" "*.Mask"))
        (pad "2" thru_hole circle (at 1 0) (size 1 1) (drill 0.5)
          (layers "*.Cu" "*.Mask"))))"#;

    #[test]
    fn public_assembly_uses_pad_envelope_without_starting_geometer() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-toon-assembly-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create test directory");
        let input = root.join("board.kicad_pcb");
        let output = root.join("output");
        fs::write(&input, SIMPLE_BOARD).expect("write board");
        let run = run_toon(&ToonOptions {
            input: Some(input),
            output: Some(output.clone()),
            pcbdoc: None,
            sides: ToonSides::Top,
            assembly: true,
            footprint: None,
            variant: None,
            all_variants: false,
            theme: None,
            soldermask_color: None,
            config: Some(root.join("toon.config")),
            write_config: None,
            workers: 1,
            cache_dir: None,
            no_cache: true,
            timings: None,
        })
        .expect("render assembly view");
        assert_eq!(run.rendered_model_count, 0);
        assert_eq!(run.geometer_request_count, 0);
        let svg = fs::read_to_string(output.join("board__toon_top.svg")).expect("read SVG");
        assert!(svg.contains("data-layer-token=\"ASSEMBLY_DESIGNATORS_TOP\""));
        assert!(svg.contains("data-component=\"J1\" data-geometry-source=\"pads\""));
        assert!(svg.contains(">J1</text>"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn public_command_applies_per_view_style_order_and_omissions() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-toon-view-config-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create test directory");
        let input = root.join("board.kicad_pcb");
        let config_path = root.join("toon.config");
        let output = root.join("output");
        fs::write(&input, SIMPLE_BOARD).expect("write board");
        let mut config = crate::toon_config::default_toon_config();
        config["views"][0]["layers"] = json!(["BOARD_OUTLINE", "BOARD_SUBSTRATE"]);
        config["views"][0]["styles"] = json!({
            "board_substrate": {"color": "#123456"},
            "soldermask_film": {"color": "#ABCDEF"}
        });
        fs::write(
            &config_path,
            serde_json::to_vec_pretty(&config).expect("serialize config"),
        )
        .expect("write config");

        let run = run_toon(&ToonOptions {
            input: Some(input),
            output: Some(output.clone()),
            pcbdoc: None,
            sides: ToonSides::Top,
            assembly: false,
            footprint: None,
            variant: None,
            all_variants: false,
            theme: None,
            soldermask_color: None,
            config: Some(config_path),
            write_config: None,
            workers: 1,
            cache_dir: None,
            no_cache: true,
            timings: None,
        })
        .expect("render configured top view");
        assert_eq!(run.rendered_model_count, 0);
        assert_eq!(run.geometer_request_count, 0);
        let svg = fs::read_to_string(output.join("board__toon_top.svg")).expect("read SVG");
        let outline = svg.find("data-layer-token=\"BOARD_OUTLINE\"").unwrap();
        let substrate = svg.find("data-layer-token=\"BOARD_SUBSTRATE\"").unwrap();
        assert!(outline < substrate);
        assert!(svg.contains("fill=\"#123456\""));
        assert!(!svg.contains("id=\"illustration-top\""));
        let manifest = fs::read_to_string(output.join("manifest.json")).expect("read manifest");
        assert!(manifest.contains("\"soldermask_color\": \"#ABCDEF\""));
        assert!(manifest.contains("\"sides\""));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn failed_promotion_restores_previous_output_and_cleans_transaction() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kicad-cruncher-toon-publish-{}-{nonce}",
            std::process::id()
        ));
        let destination = root.join("toon");
        let transaction = root.join("transaction");
        fs::create_dir_all(&destination).expect("create previous output");
        fs::create_dir(&transaction).expect("create transaction");
        fs::write(destination.join("sentinel.txt"), "previous").expect("write sentinel");

        let error = publish(&transaction.join("missing"), &destination, &transaction)
            .expect_err("missing staging directory must fail publication");
        assert!(error.to_string().contains("could not publish Toon output"));
        assert_eq!(
            fs::read_to_string(destination.join("sentinel.txt")).expect("read restored sentinel"),
            "previous"
        );
        assert!(!transaction.exists());
        fs::remove_dir_all(root).expect("remove publish test output");
    }
}
