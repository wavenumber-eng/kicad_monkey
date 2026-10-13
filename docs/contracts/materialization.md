# Materialization Contract

Status: proposed, `a0`. See
[ADR-013](../adrs/ADR-013-pcb-analytic-materialization-contract.md) and the
[design document](../design/rust-pcb-analytic-materializer.html).

A materialization describes a board as physical material: what exists, on
which layer of the stack (its slice), on which net, under which transform. It does not
carry KiCad object kinds; a separate binding table maps operations back to
their sources.

## Documents

| Document | Identifier | Schema |
|---|---|---|
| Request | `kicad_monkey.materialization.request`, version `a0` | `contracts/generated/schema/MaterializationRequest.json` |
| Result | schema `kicad_monkey.materialization.a0`, type `kicad_monkey.materialization.document` | `contracts/generated/schema/MaterializationDocument.json` |

The TypeSpec source `src/tsp/kicad_monkey/materialization-a0.tsp` is the
authority. The JSON Schemas and the Rust (`kicad-monkey-contracts`), Python
(`kicad_monkey.contracts.generated`) and TypeScript bindings are generated
from it; `npm run check:typespec`, the codegen `--check` mode and the
binding checks keep them in step.

## Request

| Field | Meaning |
|---|---|
| `products` | Any of `copper`, `holes`, `dielectric`, `solder_mask`, `paste`, `silkscreen`, `components`. |
| `slice_keys`, `net_keys` | Optional filters; an empty or absent list means all. |
| `region` | Optional board-frame bounds; operations whose bounds meet it are kept. |
| `tile_size_nm` | Chunk grid size. |
| `max_source_bytes` | Bound on the board and project files together. |
| `max_operations`, `max_points` | Bounds on the returned document. |

## Result

| Part | Content |
|---|---|
| `producer` | Producer name and version. |
| `source` | SHA-256 and size of the board file, and of the project file when one was given. |
| `coordinate_system` | Nanometres, X right and Y down in KiCad's board frame. |
| `scope` | The `products`, `slice_keys`, `net_keys` and `region` materialized, the tile size, and `complete`, true exactly when every product is named without filters. |
| `stack` | Ordered slices (key, role, order, KiCad layer, side, thickness, material, relative permittivity, loss tangent, dielectric kind), stack regions, board thickness, copper finish, impedance-control and plated-edge flags. Unknown values are absent. |
| `nets` | Resolved net keys with names and classes; `net_classes` with their rules and constituents; `diff_pairs`. |
| `components` | Reference, footprint, side, affine, source UID, body bounds, net-tie and jumper groups. |
| `operations` | Surface and hole operations, indexed from 0. |
| `binding` | One entry per operation: source UID and kind, reference, pad number, net, tied nets, pin-to-die length and delay. |
| `chunks` | Per (slice, tile): bounds, net keys and a contiguous operation range. |
| `diagnostics` | What was approximated, unsupported or not placed, with an optional product. |

### Shapes

`disk_2d`, `rect_2d`, `rounded_rect_2d`, `chamfered_rect_2d`,
`trapezoid_2d`, `capsule_2d`, `annulus_2d`, `swept_path_2d` and
`planar_region_2d`. Paths and region contours are sequences of `line`,
`arc` and `cubic` segments; a contour whose last segment does not end at
its start is closed by a straight segment. Every operation has one
`Affine2D` (2x2 linear part as `f64`, translation in nanometres).

### Operations

- Surface operation: `index`, `action` (`add` or `subtract`), `material`,
  `slice_key` (the slice's role matches the material), `net_key` (required for
  copper; the empty key is no net), `shape`, `affine`, `bounds`, optional
  `offset_nm` (a non-zero disk offset with round corners) and `group_key`.
- Hole operation: `index`, `shape` (`disk_2d` or `capsule_2d`), `affine`,
  `plating`, `start_slice_key` and `end_slice_key` (conductor slices, either
  order), `net_key` for plated holes, `plating_thickness_nm` when known,
  `protection` (per side tented, covered and plugged, then filled and
  capped, each `yes`, `no` or `unspecified`), `backdrills`,
  `post_machining`, `castellated`, `bounds`.

### Rules

1. Projected and fully parsed views of a board give identical results.
2. A selective result equals the complete result filtered to its scope.
3. Output is byte-deterministic for the same source and producer version.
4. Operations on different nets are never merged.
5. Every operation has a slice; every conductive operation has a net key.

Material combines without regard to order: per slice, per material (and
per net for copper) it is the union of the adds minus the union of the
subtracts. Operations sharing a `group_key` form one item whose subtracts
remove only its own adds.

## Pack

A pack is the canonical serialized form, written and read by
`kicad-monkey-materialization-pack`. All values are little-endian and every
section starts on 8 bytes.

| Part | Content |
|---|---|
| Preamble (16 bytes) | Magic `KMMATPK\0`, format version `0`, flags `0`, header length. |
| Header | The result as JSON without operations, binding or chunks. |
| Section directory | `u64` offset and length pairs for the string table, binding, chunk directory and linear-part table, the chunk count, then a pair per chunk block. |
| String table | Every source UID, reference, pad number and group key once. |
| Binding, chunk directory | Columns indexing the string, net and stack tables. |
| Linear-part table | Each distinct affine `m11 m12 m21 m22` once, as `f64`. |
| Column blocks | One per chunk: a struct of arrays per operation (linear-part index, translation, bounds, slice, net, group, offset, parameter start, kind, material, shape) and an `i32` parameter stream for shapes and hole details. |

The crate's module documentation gives each column's exact layout. A reader
opens the header, binding and chunk directory, checks the contract's
operation-independent rules, and decodes chunks on demand; a full decode
validates the whole document. Values round-trip exactly, including the
affines' `f64` bits. A pack is about a fifth of the JSON document's size.

## Conformance

- `src/rs/kicad-monkey-contracts/tests/materialization_validation.rs` and
  the `materialization_contract` module tests: decoding, every validation
  rule, chunking and the scope filter.
- `src/rs/kicad-monkey-materialization-pack/tests/pack.rs` and its module
  tests: exact round trips, header validation, and damaged or truncated
  packs failing closed.
- `src/rs/kicad-monkey-core/tests/board_materialization_*.rs`: producer
  behaviour and rules 1 to 3.
- `src/rs/kicad-monkey-core/examples/materialization_gate.rs`: the corpus
  gate.
