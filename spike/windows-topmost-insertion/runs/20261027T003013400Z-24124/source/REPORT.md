# Windows topmost insertion experiment

**SCHEMA 5 PREPARED, NOT RUN ON WINDOWS.** Native desktop creation, IME suppression, ownership behavior, strict adjacency, race movement and cleanup still need the first Windows run. Compilation and synthetic protocol tests do not complete that coverage. The canonical scenario matrix, environment schedule and operator workflow are in `README.md`.

## Preserved measurements

Run `runs/20261026T090428503Z-11092` measured Windows 25H2 build 26200.9711 with schema 4. It attempted 105 trials. Ninety passed setup, 89 had analyzer-eligible complete observations, and 25 explicit adjacency controls failed. Fifteen setup rejections are retained, not dismissed as irrelevant environment noise.

Normal X became topmost after insertion relative to topmost A in all five direct trials, all five with explicit preceding NotTopmost and all five after fresh demotion. An observed-normal A also became stale before queued C placement, and C subsequently became topmost. One immediate observation spanned that transition and was invalid under schema 4 even though the later samples were consistently topmost. The normal C controls did not establish queued movement, because their neighbor endpoints were unchanged.

Ten predecessor-anchor attempts did not execute because hidden IME and MSCTFIME UI windows intervened between P and X. Five true-global-sole attempts did not execute because other flagged windows existed. Historical case 8 measured the lowest globally flagged A with 129 normal windows above it. That is not a clean contiguous topmost boundary.

Read-only replay with the current legacy analyzer is byte-identical to this run's embedded analyzer output. Raw SHA-256 remains `62de6552ce92933a0ca37dcef2fd7eefe578457c995dc575cb98205a3abecb4f`. That historical hash fixes the reviewed local copy, not native-to-local transfer fidelity. No prior run, raw capture, generated report or preserved artifact was rewritten.

The earlier all-setup-rejected capture diagnosed an enumeration-order assumption. Schema 3 and later reconstruct the actual linked chain instead of treating EnumWindows callback order as immediate-neighbor order. That correction did not establish atomic snapshots or erase the later native eligibility failures.

## Implementation rationale

The existing process/job/stdio protocol remains. No generic native process launcher was added. The supervisor creates the private desktop in its current window station and restores its borrowed original desktop before continuing. A fresh environment thread explicitly attaches before creating X. Each fresh fixture process opens and attaches to the named desktop before window creation. Children use CREATE_NO_WINDOW to avoid fixture console windows.

IME suppression addresses the measured hidden-window obstruction without changing baseline window styles. It does not promise that Text Services Framework creates no companion. Strict raw adjacency remains the acceptance criterion. The private desktop isolates the otherwise-unattainable global premises without changing shell or unrelated application windows. Its new clean-prefix predicate does not reinterpret historical lowest-flagged evidence.

The race fixture makes P an independent movement witness. A's promotion and demotion can change C's neighbors without executing C's queued placement. C crossing P only after release is stronger evidence than a normal flag or an unchanged destination. No extra event framework or application restacking is introduced.

Owned-window identity is scheduled explicitly rather than inferred from a matching process. This allows O to share its owner's native thread without weakening baseline independence. Relative insertion records both strict adjacency and owner-group order because an owned popup may legitimately intervene. Bottom requires a full chain proof rather than a local absence of a known successor.

Schema 5 separates non-atomic snapshot rejection from endpoint resolution. Earlier invalid slots remain invalid. A later plateau counts only at the fixed final scheduled slots, so missing convergence cannot be repaired by waiting longer or replacing samples. The legacy analyzer path is unchanged.

## API basis

- [ImmDisableIME](https://github.com/MicrosoftDocs/sdk-api/blob/docs/sdk-api-src/content/imm/nf-imm-immdisableime.md) documents 0 for the current thread, -1 for every process thread, success as TRUE, and invocation before the first top-level WM_CREATE.
- [CreateDesktopW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createdesktopw) documents current-window-station association, calling-thread assignment and required DESKTOP_CREATEWINDOW access. The suite leaves security attributes unset and requests no switch right.
- [OpenDesktopW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-opendesktopw) and [SetThreadDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop) support explicit attachment before the fresh thread owns windows or hooks. Attachment failure is recorded, never replaced by Default.
- [SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos) documents that promoting an owner promotes owned windows without promoting its own owner. Demoting a topmost window also demotes its owners and owned windows. A non-topmost owner can own a topmost window. Bottom removes topmost status and places the target below all other windows. These statements define ownership and Bottom controls, while raw outcomes remain independently reported.

No documentation conflict required dropping a requested row. Whether these APIs and requested desktop rights work on the operator's actual machine remains a native preflight question.

## Verification limits

Local host tests exercise fixture relationship validation, clean-prefix rejection and the fixed schedule. Python protocol tests cover every synthetic scenario and scheduled combination, owned-window admission without baseline relaxation, private-preflight not-run records, missing/duplicate keys, non-atomic immediate observations, the fixed final-five rule and false queued-movement endpoints. These are protocol tests, not simulations that prove Win32 behavior.

Host and Windows GNU compilation and Clippy are checked separately. Neither executes the Windows APIs here. No desktop was opened and no native run was made during this revision. Source embedding, native raw hashing, actual desktop access, visible-window isolation, IME suppression and native teardown remain first-run checks. No result from this bounded fixture suite could establish every possible Windows behavior.
