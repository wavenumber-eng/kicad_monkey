+++
type = "plan_log"
id = "kicad-monkey-performance-optimization-sweep-2026-07-17-regex-tokenizer-performance-summary"
plan_id = "kicad-monkey-performance-optimization-sweep"
step_id = "post-regex-behavior-and-performance-signoff"
created = "2026-07-17T09:42:00-04:00"
+++

# Regex Tokenizer Performance Summary

Comparison uses `best_s` from the original baseline, the pre-regex performance
signoff, and the post-regex signoff JSON. The regex signoff used three rounds
over synthetic cases plus the three largest local corpus boards.

The regex tokenizer rewrite is behavior-preserving and improves most projection
hydration workloads beyond the pre-regex signoff. It does not deliver the
earlier 3-8x full-parse expectation: synthetic full parse is effectively
neutral, Jumperless full parse is neutral, 4-ch full parse improves, and WREN
full parse is slower than the pre-regex signoff on this run. Treat the full
parse ceiling as a remaining review topic rather than a closed claim.

| Case | Operation | Baseline best_s | Pre-regex best_s | Regex best_s | Speedup vs baseline | Delta vs pre-regex |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | full_parse | 10.432 | 8.095 | 7.672 | 1.36x | -5.2% |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | projection_common_families | 11.871 | 10.288 | 8.782 | 1.35x | -14.6% |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | projection_nested | 38.420 | 9.486 | 8.295 | 4.63x | -12.6% |
| corpus:4-ch-backplane/4-ch-backplane.kicad_pcb | projection_routes | 6.070 | 4.397 | 4.166 | 1.46x | -5.3% |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | full_parse | 44.412 | 25.160 | 26.650 | 1.67x | +5.9% |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | projection_common_families | 44.410 | 35.344 | 32.560 | 1.36x | -7.9% |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | projection_nested | 126.520 | 22.523 | 20.859 | 6.07x | -7.4% |
| corpus:cern_wren_eda_04903/EDA-04903-V1-0.kicad_pcb | projection_routes | 36.382 | 20.121 | 19.979 | 1.82x | -0.7% |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | full_parse | 15.686 | 11.959 | 12.020 | 1.31x | +0.5% |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | projection_common_families | 19.649 | 16.097 | 14.579 | 1.35x | -9.4% |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | projection_nested | 33.024 | 10.293 | 8.974 | 3.68x | -12.8% |
| corpus:jumperless_v5r7/JumperlessV5r7.kicad_pcb | projection_routes | 12.847 | 9.789 | 9.236 | 1.39x | -5.6% |
| synthetic-nested-spans | full_parse | 0.376 | 0.338 | 0.339 | 1.11x | +0.2% |
| synthetic-nested-spans | projection_common_families | 0.626 | 0.432 | 0.423 | 1.48x | -2.0% |
| synthetic-nested-spans | projection_nested | 1.880 | 0.562 | 0.549 | 3.42x | -2.3% |
| synthetic-nested-spans | projection_routes | 0.110 | 0.087 | 0.090 | 1.22x | +3.5% |
| synthetic-net-dense | full_parse | 0.506 | 0.333 | 0.343 | 1.48x | +2.9% |
| synthetic-net-dense | projection_common_families | 0.599 | 0.409 | 0.423 | 1.41x | +3.4% |
| synthetic-net-dense | projection_nested | 0.572 | 0.277 | 0.278 | 2.05x | +0.6% |
| synthetic-net-dense | projection_routes | 0.397 | 0.256 | 0.272 | 1.46x | +6.5% |
| synthetic-top-level-scan | full_parse | 0.214 | 0.123 | 0.124 | 1.73x | +0.7% |
| synthetic-top-level-scan | projection_common_families | 0.326 | 0.153 | 0.157 | 2.08x | +2.1% |
| synthetic-top-level-scan | projection_nested | 0.098 | 0.042 | 0.042 | 2.30x | +1.1% |
| synthetic-top-level-scan | projection_routes | 0.106 | 0.046 | 0.046 | 2.31x | +1.1% |

Evidence files:

- Baseline: `docs/plans/logs/2026-07-17-performance-baseline.json`.
- Pre-regex signoff: `docs/plans/logs/2026-07-17-performance-signoff.json`.
- Regex signoff: `docs/plans/logs/2026-07-17-regex-tokenizer-performance-signoff.json`.
- Synthetic regex probe: `docs/plans/logs/2026-07-17-regex-tokenizer-synthetic-probe.json`.
