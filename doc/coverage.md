# 📊 Release coverage — Experiment 011

Measured using cargo-llvm-cov 0.9.1 on Linux/WSL with the final 0.2.0 source.
Separate coverage build directories prevent stale binary/source mappings.

| Scope | Lines | Functions | Regions |
| --- | --- | --- | --- |
| New production middleware | 206/206 (100.00%) | 30/30 (100.00%) | 293/293 (100.00%) |
| Whole workspace, including real PostgreSQL tests | 2097/2369 (88.52%) | 313/373 (83.91%) | 3209/3675 (87.32%) |

✅ The middleware gate requires 100% line/function/region coverage of its actual
production source; the source is not hidden or excluded. The workspace result
is deliberately reported separately and is not claimed to be 100%. Generic
instantiation coverage is a different metric, and branch instrumentation is
not enabled by these commands. Coverage does not prove every possible runtime
case is correct.

The middleware suite contains 15 integration tests covering boundaries,
concurrency, errors, CORS, deadlines, tracing context and configuration.
The measured workspace corpus runs regular tests and the five normally ignored
PostgreSQL cases against an owned disposable test server.

See [testing commands](testing_guid.md#-measured-coverage) and the
[middleware guide](middleware_guid.md) for reproducible checks and behavior limits.
