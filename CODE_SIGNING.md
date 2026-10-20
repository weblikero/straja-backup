# Code signing policy

Status: preparation for a SignPath Foundation application, 2026-10-20.
No application has been submitted or accepted as part of this preparation.
Published Windows kits through 3.0.5 are unsigned. Do not interpret this page
as evidence that a downloaded file has a trusted signature.

Straja Backup is maintained by [Weblike PRO](https://weblike.ro/), with source
at [weblikero/straja-backup](https://github.com/weblikero/straja-backup).

## Proposed signing responsibility

The repository owner, [weblikero](https://github.com/weblikero), is the proposed
committer, reviewer and release-signing approver. Account security and SignPath
role assignments must be confirmed during onboarding. Approval must remain a
deliberate maintainer action.

## Intended Windows release process

Build the GUI, agent and installer from the reviewed release commit. Preserve
the CI run and artifact identity so the signing service can verify their origin.
Sign Straja-owned executables, package them, then sign the installer using the
approved integration. Verify the uninstaller's signature as well. Recompute
package checksums after signing and test the resulting installer before release.

Restic and Rclone are separately maintained upstream components, distributed
with their notices. They must not be represented as Straja-owned code or signed
individually under the proposed Straja signing policy.

Keep published release files immutable. A newly signed release gets a new
version; it does not silently replace the unsigned 3.0.5 downloads.

## Provider status and attribution

If SignPath Foundation accepts the project, its certificate identifies
**SignPath Foundation** as publisher. After acceptance and successful integration,
activate the attribution:

> Free code signing provided by SignPath.io, certificate by SignPath Foundation

That attribution is conditional here, not a claim of current sponsorship.
See the [provider's conditions](https://signpath.org/terms) and the
[onboarding checklist](SIGNPATH_APPLICATION.md).

## Privacy

See the [desktop application privacy policy](PRIVACY.md), including automatic
GitHub update requests and optional connections to backup providers.
