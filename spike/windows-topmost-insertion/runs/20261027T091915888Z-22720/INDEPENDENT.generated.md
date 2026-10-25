# Independent verification of ownership follow-up

This is generated analysis of run 20261027T091915888Z-22720 on Windows 25H2 build 26200.9711. VERIFICATION.generated.md holds the full matrix. Raw records and captured source remain unchanged.

## Supported observations

All 245 scheduled trials are present. There are 235 collected endpoints, 215 intended questions observed, 20 failed-prefix measurements and ten setup rejections. The analyzer reports zero protocol-invalid trials and 45 contradicted reference expectations. A contradicted expectation is not automatically invalid native evidence, and a measured endpoint after a failed prefix does not establish the intended question.

Cases 30, 31 and 32 verify X topmost before creating owned O, with no explicit topmost creation style on O. All 30 creation sequences establish both flags before treatment. Demoting X then demotes both X and O on both desktops, five times each.

Demoting O differs by environment. On the ordinary desktop both become normal, five times. On the private desktop O becomes normal while X remains topmost immediately above O, five times. Actual ownership and reciprocal neighbor links persist across every final reread. The only measured native write targets O with NotTopmost. This is a scoped observation, not a universal owner rule or proof that desktop inactivity causes the difference.

Inserting the verified topmost owner X after A keeps both X and O topmost and produces A-O-X-B-P on both desktops. O prevents strict X adjacency. Both fixed explicit promotion orders also establish both flags and allow owner demotion to demote both.

Clean-boundary case 8 and normal Bottom case 28 now demonstrate actual movement across stationary P in all five private trials. Their earlier unchanged-position limitation is closed.

## The flag comparison is confounded

Unequal initial order and creation history prevent attributing the paired outcomes solely to NOOWNERZORDER. Cases 22 and 26 start A-B-P-O-X. Cases 35 and 36 start A-B-O-X-P. All setup calls use flags 19, but resetting bands does not erase prior order. X is reused within its environment. Case 36 also creates O after the previous trial left X topmost, unlike case 26, which creates O while X is normal. A later demotion makes their bands equal without making their histories equal.

Under flags 531, case 35 promotes X alone and leaves O normal below it. Case 36 retains that split prefix and places X directly below A. These are outcomes under the NOOWNERZORDER condition, not effects attributable solely to that flag. Case 33 supplies a flags-19 promotion from the same controlled order that already makes both X and O topmost, but with a different setup history. Flags 19 therefore do not universally produce case 22's owner-normal/owned-topmost outcome.

A causal flag-only comparison would require identical precreation history and paired initial order. It is not needed to preserve the measured insertion, boundary, delay or scoped ownership results. No further native run was performed during verification.

## Integrity and limits

Independent parsing accounts for 1,470 distinct native calls and 20 identical reply/treatment duplicates. Every call has the prescribed target, anchor, flags, caller and stage bounds, with no overlapping, cross-lifetime or post-endpoint write. All native calls report success. All 865 required target reads match their identities and end before their following collector read.

There are 235 final settlement groups containing 4,935 samples, plus five first-settlement groups for repeated insertion. Every final-five group agrees on all fixture fields. Seven invalid post-release snapshots remain recorded and are not settlement samples. Forty private global snapshots contain complete unfiltered chains with exactly their scheduled fixtures.

Raw SHA-256 is `afb0d2692693951d2b5ba1e3f99305080cf80d126299f1f1d98d03ee7bfa97a3`, matching the native hash over 36,552,997 bytes. The executable hash matches. All 20 captured source manifest entries match after CRLF reconstruction in memory, and those bytes occur in the executable. Files were not normalized on disk.

Desktop assignment and IME disabling precede creation. Private closure and job cleanup report success, with zero remaining job processes and empty native stderr. Source contains no SwitchDesktop call. These checks are not visual proof or individual native destruction acknowledgments. Runtime tool versions are not compiler provenance.
