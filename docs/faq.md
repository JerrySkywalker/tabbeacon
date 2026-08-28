# Frequently asked questions

## Does TabBeacon wrap Codex or Agy?

No. Daily commands remain literally `codex` and `agy`. TabBeacon does not add a
launcher, fake executable, PATH shadow, PTY host, or global daemon baseline.

## Does the daily command change?

No. Set up the admitted integration once, then use the provider's own command.

## Does TabBeacon read prompts?

Normal presentation does not ingest or persist prompt, assistant, or tool
content. Bounded diagnostics expose operational facts rather than credentials,
raw session identifiers, environment dumps, or transcripts.

## Does normal workspace identity require the Internet?

No. Workspace identity is offline-first, with Git identity used as a stable
specialization when available.

## Why is Hook trust manual?

Trust belongs to the provider and the Owner. Compatibility evidence and setup
ownership cannot grant it automatically.

## Why can an upgrade require sessions to exit?

Windows may prevent replacing an in-use executable. TabBeacon distinguishes
exact owned workers from unknown processes and only offers an explicit drain
after fresh ownership proof.

## Why are ambiguous processes preserved?

Because an image name, command text, window order, or PID alone does not prove
ownership. Preserving ambiguity avoids stopping unrelated work.

## Why is Codex capability-based?

A version number is not proof of Hook availability or behavior. TabBeacon uses
local required-capability evidence so an unseen version can be treated honestly
as Full, Degraded, Incompatible, or Unproven.

## Why is Agy support narrower?

The admitted Agy 1.1.19 profile provides a smaller, structured title callback
contract. TabBeacon does not invent lifecycle or presentation facts it cannot
prove from that contract.

## Why is a native tab icon difficult?

Stock Windows Terminal has an internal icon pipeline but no supported public
application-controlled tab-icon bridge. The remaining documented diagnostic
route is process-scoped instrumentation and could not be safely isolated on the
accepted host.

## Does v0.7 include native tab icons?

No. Native Tab Icon is **NO_GO** under accepted current-host safety evidence;
`TitleMarkBackend` remains the production presentation path.

## Are Claude Code or OpenCode supported?

No. Both integrations are Deferred, not partially supported.
