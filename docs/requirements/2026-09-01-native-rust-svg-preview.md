# Native Rust SVG Preview Requirements

## Status

Accepted for implementation on 2026-09-01; release acceptance is pending.

## Public outcome

An external Rust program must be able to parse/project and render ordinary
`.kicad_mod`, selected `.kicad_sym`, `.kicad_pcb`, and concrete `.kicad_sch`
page occurrences through linked `kicad-monkey-core` and `kicad-monkey-svg`
APIs. The runtime path must not import WASM, execute a native sidecar, invoke
Python, or require JSON serialization between projection and rendering.

The public surface consists of symmetric typed projectors, four direct render
entry points, a validated versioned context, explicit-or-fit viewport policy,
typed bounds, resource limits, metrics, and structured errors. Complete
schematic page composition must accept a `SourceBundle` plus an unambiguous
occurrence selector and all explicit drawing, worksheet, variable, and font
resources.

## Compatibility

- The four frozen Plotter-IR a0 schemas and native SVG a0 request/result remain
  unchanged.
- The default direct context with an explicit viewport is byte-identical to
  every accepted native a0 success vector.
- Every MOD, SYM, PCB, and SCH direct-render entry point consumes the same
  validated render context. Color remaps, semantic/layer/operation style
  overrides, text policy, visibility, identity emission, and viewport/fit
  policy take effect consistently wherever represented; no family silently
  ignores a supported override.
- Existing WASM and native requests retain identities, diagnostics, limits,
  output accounting, and deterministic behavior.
- The reviewed source corpus, Yoshi source, and authoritative default fixture
  outputs are not rewritten. Additive filtered/themed outputs use new names.

## Visibility and fit

Render-time schematic visibility can suppress retained pin names and numbers.
Hidden-pin and hidden-field inclusion remains a projector input: frozen a0 does
not retain a truthful hidden discriminator, so the SVG context must not pretend
it can recover or distinguish those items after projection.

One bounded effective-operation predicate feeds both bounds and SVG emission.
Exact layer names and the documented bounded wildcard dialect are supported.
Unknown or invalid selectors fail unless permissive behavior is explicitly
requested. Record and operation layers compose without losing balanced block
ownership or placement transforms.

Pad copper uses exact membership: `F&B.Cu` means the two outer layers only.
Via apertures use their already-resolved flash layers; endpoint tokens are not
expanded into an aperture span. Via drills expand between endpoints over the
board copper stack. Plated pad drills remain physical through-board evidence
when inner flashes are absent, and NPTH cutouts are independent of copper
visibility. Mask, paste, silk, fab, courtyard, and other represented layers use
their exact names.

Fit bounds use the same validated context, visibility decisions, effective
stroke widths, and transforms as rendering. Empty visible content produces a
typed error unless the caller supplied a fallback. Padding, aspect handling,
minimum extent, arithmetic, traversed geometry, and output are bounded.
Fit must reject visible semantic text without deterministic retained contour
geometry. It must not estimate arbitrary browser-font glyph bounds. Explicit
viewport rendering retains current semantic-text behavior.

Board layer filtering receives an additive typed, non-wire complete enabled
layer catalog and ordered copper stack produced atomically with the board
document. Private constructors bind the facts through source and projected
artifacts. This sidecar is required for strict via-drill span selection and
unknown-layer diagnostics; frozen a0 is not widened. Typed-document-only board
rendering permits only default all-layer visibility. Layer matching is ASCII
case-sensitive and accepts only exact names, `*`, or one leading-star suffix
such as `*.Cu`. No whitespace/case normalization is performed.

Complete schematic rendering consumes the occurrence-bearing page artifact and
returns its occurrence address. The lower-level typed-document renderer carries
no occurrence identity rather than guessing one.

Layer scope is resolved per typed record family before both bounds and SVG
emission. Homogeneous carriers lend record layers only to layerless children;
embedded-footprint children use their own/block layers and never the placement
side as a gate; via roles determine aperture/drill scope; and each zone ring
retains its corresponding fill-layer evidence. Unlayered fallback is explicit.

## Resource and dependency requirements

Projection independently bounds source/retained strings, records, operations,
points, nested items, and estimated materialized memory before publication.
Rendering independently bounds records, operations, points, text/image bytes,
block depth, bounds work, elements, serialization work, SVG bytes, context
entries, and context string bytes. Exact-limit and one-under tests are required.

Dependency direction is `svg -> core -> contracts`; WASM and native transport
are leaves. Monkey never depends on Cruncher or Alexandria. A temporary
external Cargo project must resolve all directly used Monkey crates from one
exact Git revision and prove that its runtime tree excludes WASM/native.

## Acceptance evidence

Acceptance requires four-family projector tests, direct-render byte parity,
four-family non-default context/override tests, context validation, bounds/fit,
Yoshi pad/via/drill assertions, repeated-child
schematic occurrence selection, real Node WASM and native transport parity, a
pinned-Git consumer, relevant Rack/corpus/Cruncher gates, locked workspace
tests, formatting, warning-denied Clippy, dev-std audits, and independent
architecture/implementation/resource/test review.

Performance evidence records cold and warm build time plus render time, peak
memory, and SVG bytes for at least one fixture per family. It is a regression
baseline, not a cross-machine SLO.
