use std::sync::Arc;
use std::time::{Duration, Instant};

use kicad_monkey_core::{
    BoardPlotSourceArtifact, PlotDocumentMetadata, PlotDocumentProjectionLimits,
    PlotProjectionError, ProjectedBoardPlotArtifact, project_board_plot_artifact_a0,
};
use kicad_monkey_svg::{
    SvgArtifact, SvgError, SvgErrorKind, SvgFitOptions, SvgRenderLimits, SvgViewport,
    ValidatedSvgRenderContextA1, ViewportPolicy, render_board_svg,
};

/// Measured boundaries, kept separate from user-facing progress messages.
#[derive(Clone, Copy, Debug, Default)]
pub struct PcbSvgJobProfile {
    pub projection: Duration,
    pub rendering: Duration,
    pub render_requests: usize,
    pub render_cache_hits: usize,
    pub retained_svg_bytes: usize,
}

struct CachedRender {
    context: ValidatedSvgRenderContextA1,
    viewport_policy: ViewportPolicy,
    artifact: Arc<SvgArtifact>,
}

/// A job owns one typed board projection and reuses equal physical render requests.
///
/// Context comparison includes every presentation option, so a style or visibility
/// change cannot incorrectly reuse a previous layer. Callers retain the same job
/// across views/variants; changed source geometry requires a new job.
pub struct PcbSvgJob {
    plot: ProjectedBoardPlotArtifact,
    renders: Vec<CachedRender>,
    limits: SvgRenderLimits,
    max_retained_svg_bytes: usize,
    profile: PcbSvgJobProfile,
}

impl PcbSvgJob {
    pub fn new(
        source: BoardPlotSourceArtifact,
        metadata: PlotDocumentMetadata,
        limits: SvgRenderLimits,
    ) -> Result<Self, PlotProjectionError> {
        let start = Instant::now();
        let plot = project_board_plot_artifact_a0(
            source,
            metadata,
            PlotDocumentProjectionLimits::default(),
        )?;
        Ok(Self {
            plot,
            renders: Vec::new(),
            max_retained_svg_bytes: limits.max_svg_bytes,
            limits,
            profile: PcbSvgJobProfile {
                projection: start.elapsed(),
                ..Default::default()
            },
        })
    }

    /// Borrow the plotter operations and source layer facts without JSON/XML parsing.
    pub fn plot(&self) -> &ProjectedBoardPlotArtifact {
        &self.plot
    }

    pub fn profile(&self) -> PcbSvgJobProfile {
        self.profile
    }

    pub fn render_physical(
        &mut self,
        viewport: SvgViewport,
        context: &ValidatedSvgRenderContextA1,
    ) -> Result<Arc<SvgArtifact>, SvgError> {
        self.render_physical_with_policy(ViewportPolicy::Explicit(viewport), context)
    }

    pub fn render_physical_fit(
        &mut self,
        fit: SvgFitOptions,
        context: &ValidatedSvgRenderContextA1,
    ) -> Result<Arc<SvgArtifact>, SvgError> {
        self.render_physical_with_policy(ViewportPolicy::Fit(fit), context)
    }

    pub fn render_footprint(
        &mut self,
        reference: &str,
        viewport: SvgViewport,
        context: &ValidatedSvgRenderContextA1,
    ) -> Result<Arc<SvgArtifact>, SvgError> {
        self.render_footprint_with_policy(reference, ViewportPolicy::Explicit(viewport), context)
    }

    pub fn render_footprint_fit(
        &mut self,
        reference: &str,
        fit: SvgFitOptions,
        context: &ValidatedSvgRenderContextA1,
    ) -> Result<Arc<SvgArtifact>, SvgError> {
        self.render_footprint_with_policy(reference, ViewportPolicy::Fit(fit), context)
    }

    fn render_footprint_with_policy(
        &mut self,
        reference: &str,
        viewport_policy: ViewportPolicy,
        context: &ValidatedSvgRenderContextA1,
    ) -> Result<Arc<SvgArtifact>, SvgError> {
        self.profile.render_requests += 1;
        let selected = self.plot.select_footprint_reference(reference);
        let start = Instant::now();
        let result = render_board_svg(&selected, viewport_policy, context, self.limits);
        self.profile.rendering += start.elapsed();
        result.map(Arc::new)
    }

    fn render_physical_with_policy(
        &mut self,
        viewport_policy: ViewportPolicy,
        context: &ValidatedSvgRenderContextA1,
    ) -> Result<Arc<SvgArtifact>, SvgError> {
        self.profile.render_requests += 1;
        if let Some(cached) = self
            .renders
            .iter()
            .find(|entry| entry.viewport_policy == viewport_policy && entry.context == *context)
        {
            self.profile.render_cache_hits += 1;
            return Ok(Arc::clone(&cached.artifact));
        }
        let start = Instant::now();
        let result = render_board_svg(&self.plot, viewport_policy, context, self.limits);
        self.profile.rendering += start.elapsed();
        let artifact = Arc::new(result?);
        let bytes = artifact.svg.len();
        if bytes > self.max_retained_svg_bytes {
            return Err(SvgError::new(
                SvgErrorKind::ResourceLimit,
                "physical SVG exceeds job cache budget",
            ));
        }
        // A bounded FIFO retains the common layer set; eviction changes work only.
        while self.profile.retained_svg_bytes > self.max_retained_svg_bytes - bytes
            || self.renders.len() >= 128
        {
            let removed = self.renders.remove(0);
            self.profile.retained_svg_bytes -= removed.artifact.svg.len();
        }
        self.profile.retained_svg_bytes += bytes;
        self.renders.push(CachedRender {
            context: context.clone(),
            viewport_policy,
            artifact: Arc::clone(&artifact),
        });
        Ok(artifact)
    }
}
