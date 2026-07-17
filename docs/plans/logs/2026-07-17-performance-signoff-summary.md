+++
type = "plan_log"
id = "kicad-monkey-performance-optimization-sweep-2026-07-17-performance-signoff-summary"
plan_id = "kicad-monkey-performance-optimization-sweep"
step_id = "performance-signoff"
created = "2026-07-17T08:55:00-04:00"
+++

# Performance Signoff Summary

Comparison uses `best_s` from the committed first-pass baseline and final signoff JSON.
The baseline used 1 round; final signoff used 3 rounds over synthetic cases plus the three largest local corpus boards.

| Case | Operation | Baseline best_s | Final best_s | Reduction | Speedup |
| --- | --- | ---: | ---: | ---: | ---: |
| synthetic-net-dense | full_parse | 0.506 | 0.333 | 34.1% | 1.52x |
| synthetic-net-dense | projection_routes | 0.397 | 0.256 | 35.6% | 1.55x |
| synthetic-net-dense | projection_nested | 0.572 | 0.277 | 51.6% | 2.07x |
| synthetic-net-dense | projection_common_families | 0.599 | 0.409 | 31.6% | 1.46x |
| synthetic-nested-spans | full_parse | 0.376 | 0.338 | 10.1% | 1.11x |
| synthetic-nested-spans | projection_routes | 0.110 | 0.087 | 21.0% | 1.27x |
| synthetic-nested-spans | projection_nested | 1.880 | 0.562 | 70.1% | 3.35x |
| synthetic-nested-spans | projection_common_families | 0.626 | 0.432 | 31.1% | 1.45x |
| synthetic-top-level-scan | full_parse | 0.214 | 0.123 | 42.5% | 1.74x |
| synthetic-top-level-scan | projection_routes | 0.106 | 0.046 | 57.2% | 2.34x |
| synthetic-top-level-scan | projection_nested | 0.098 | 0.042 | 56.9% | 2.32x |
| synthetic-top-level-scan | projection_common_families | 0.326 | 0.153 | 52.9% | 2.12x |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | full_parse | 44.412 | 25.160 | 43.3% | 1.77x |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | projection_routes | 36.382 | 20.121 | 44.7% | 1.81x |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | projection_nested | 126.520 | 22.523 | 82.2% | 5.62x |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | projection_common_families | 44.410 | 35.344 | 20.4% | 1.26x |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | full_parse | 10.432 | 8.095 | 22.4% | 1.29x |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | projection_routes | 6.070 | 4.397 | 27.6% | 1.38x |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | projection_nested | 38.420 | 9.486 | 75.3% | 4.05x |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | projection_common_families | 11.871 | 10.288 | 13.3% | 1.15x |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | full_parse | 15.686 | 11.959 | 23.8% | 1.31x |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | projection_routes | 12.847 | 9.789 | 23.8% | 1.31x |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | projection_nested | 33.024 | 10.293 | 68.8% | 3.21x |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | projection_common_families | 19.649 | 16.097 | 18.1% | 1.22x |

Notes:

- Synthetic cases are the clean public/reproducible optimization evidence.
- `corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb` and `corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb` are public-corpus evidence.
- `corpus:4-ch-backplane/4-ch-backplane.kicad_pcb` is local/internal corpus evidence and should not be the only release-facing support for claims.
- Final signoff JSON: `docs/plans/logs/2026-07-17-performance-signoff.json`.
- Baseline JSON: `docs/plans/logs/2026-07-17-performance-baseline.json`.
