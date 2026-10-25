# Independent verification of schema 5 results

This is generated analysis of the 2026-10-27 Windows experiment, not collector output. Read REPORT.generated.md for the full matrix and raw line anchors. Native output, source and hash records remain unchanged. The experiment ran on Windows 25H2 build 26200.9711, with ordinary and private desktop results kept separate.

## Verified results

All 165 scheduled attempts are present. There are 155 collected trials, 150 analyzer-eligible observations, five failed owner-promotion controls, ten setup rejections and five later insertion-premise failures. There are 145 eligible observations without failed required controls. This is not an all-controls-pass run.

On the private desktop, normal X moved directly below the genuinely sole global topmost A and remained normal in five repetitions. The clean two-topmost boundary case likewise left X normal in five repetitions, but X already occupied its destination. Interior insertion with another topmost window below A promoted X in five repetitions on each desktop. Comparison controls agree across desktops within the tested scope.

The normal predecessor anchor case succeeded on the ordinary desktop. Normal P immediately preceded normal X. Inserting topmost A after P produced P-A-X with A normal, five times. With topmost P at the private desktop boundary, the same relative arrangement left A topmost, five times. These are synchronous fixture operations, not proof of a production asynchronous ordering barrier.

All three queued-C cases now demonstrate actual movement across stationary P. C began below P, remained there while held, and crossed above P directly after A following release. C ended topmost when A had become topmost, and normal when A stayed or returned normal. Each outcome occurred five times. Invalid immediate snapshots remain invalid, while the final five scheduled samples are consistent. The experiment did not replace samples or extend collection until success.

## Ownership remains incomplete

With O already owned by X, the recorded X Topmost call left X normal and O topmost in five eligible case-22 trials. This failed the expected owner-promotion control. The native call really targets X with HWND_TOPMOST. Handle conversion, actual ownership, process/thread identity, source call path and absence of a later demoting write were checked. The capture does not explain Windows' internal cause.

The same result prevented cases 23 and 25 from establishing both X and O topmost. All ten attempts rejected setup, so their explicit demotion operations were not tested. Case 26 failed its later X-topmost insertion premise but still executed the relative insertion. Its endpoints are recorded but are not eligible evidence of the intended topmost-owner insertion.

Promoting O left X normal in five case-24 trials. In case 27, inserting A after normal X demoted both A and its owned O, producing X-O-A. Strict adjacency and owner-group ordering are not interchangeable.

Promoting X before creating O is a candidate for establishing the missing demotion premise. It has not been tested for this fixture and must not be called a verified repair. Existing cases 22 and 26 must remain preserved rather than being rewritten as successes.

## Scope and integrity

All 970 distinct SetWindowPos calls reported success. All 220 measured-interval calls have the intended target, anchor, flags, caller identity and timestamp bounds. All 160 settlement groups, including repeated insertion, have consistent valid final-five fixture observations. Ten intermediate observations are invalid and remain recorded.

Baseline IME disabling succeeded on both coordinator threads and 21 child threads. All 187 inventories contain only scheduled fixture identities. Global desktop evidence was not filtered to hide companions. Actual private desktop identity matches across its coordinator and children. Source contains no SwitchDesktop invocation. Private desktop closure and job cleanup reported success, with zero remaining job processes. This is not visual proof or individual DestroyWindow acknowledgment.

Raw SHA-256 is `07190550967db31935c341061c0caff5ffc97509f190226770ee8dd304eba431`, matching the native post-collection hash. The executable matches its native hash. All 17 captured source files match their manifest after CRLF reconstruction only in memory, and those reconstructed byte sequences occur in the executable. Runtime PATH tool versions are not compiler provenance.
