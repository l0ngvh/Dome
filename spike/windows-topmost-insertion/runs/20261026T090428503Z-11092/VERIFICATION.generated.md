# Independently verified topmost insertion evidence

Personal Dome experiment collected on 2026-10-26, on one interactive Windows desktop reporting 25H2 build 26200.9711. This document is analysis, not native output. Read REPORT.generated.md for the matrix and exact raw line anchors, raw.jsonl for observations and calls, and source/ for the captured collector and analyzer. The supplied source and executable are preserved for reproducing the tested operations.

## Verified conclusions

A normal coordinator-owned window X gained WS_EX_TOPMOST after synchronous SetWindowPos insertion after a topmost application A, with another topmost application B below A. This occurred in all five repetitions of the direct case, all five with explicit NotTopmost immediately beforehand, and all five with X freshly demoted from topmost. These are measured counterexamples to treating prior demotion as protection against subsequent relative insertion.

Reading A as normal before posting asynchronous insertion of a separate-process window C did not protect C from promotion when A became topmost before C resumed its message loop. Four sequences have consistent immediate observations. The fifth immediate snapshot spans a transition and is invalid, but its later 21 samples consistently show topmost C. No other C placement was recorded in that interval. This is not instrumentation of the internal Windows dispatch point and is not an ordering guarantee for other applications.

## Limits that must travel with the result

There were 105 attempts, 90 accepted setups, 89 analyzer-eligible complete observations and 25 failed adjacency controls. This was not an all-tests-pass run. The failed adjacency controls do not erase independently checked style-bit measurements.

Ten anchor attempts were rejected because hidden IME and MSCTFIME UI windows owned by X intervened between the intended predecessor and X. Five sole-global-topmost attempts were rejected because other flagged windows existed. A true sole-global-topmost insertion remains untested.

The lowest-globally-flagged-A case promoted X in five repetitions, but 129 normal windows were above A. It does not establish a clean contiguous band boundary. The normal C controls observed no release-time movement and cannot prove queued execution from an unchanged endpoint.

All observations are non-atomic. The evidence applies to this host, these exact operations and controlled unowned fixture parents with observed owned companions. Arbitrary application restacking, focus, owned application targets and notification ordering remain untested.

## Integrity

Raw SHA-256 is `62de6552ce92933a0ca37dcef2fd7eefe578457c995dc575cb98205a3abecb4f`.

Executable SHA-256 is `3e4f502c199504c641c5677b480fe36acfffd733e148e4374c257e3392f36714`.

Every captured source file is LF-normalized in the pulled copy. Reconstructing CRLF in memory matches each native manifest hash, and each reconstructed complete source byte sequence occurs in the verified executable. Source files were not rewritten to hide this difference. Runtime PATH tool versions are not compiler provenance. A native hash of raw.jsonl was not captured, so the local raw hash fixes the reviewed copy rather than proving transfer fidelity.
