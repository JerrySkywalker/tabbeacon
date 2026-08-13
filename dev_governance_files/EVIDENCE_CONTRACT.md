# Evidence Contract

TabBeacon treats evidence as a first-class build artifact.

## Required identity fields

Every governed validation record should include, where applicable:

```text
RUN_ID=
GOAL_ID=
EXPECTED_HEAD=
CODE_HEAD=
VISUAL_HEAD=
RUNNER_NAME=
RUNNER_CLASS=
```

## Required disposition fields

```text
DISPOSITION=<PASS|FAIL|BLOCKED|UNPROVEN>
BLOCKED_STAGE=
BLOCKED_REASON=
FAILURE_CATEGORY=
OWNER_ACTION=
```

Fields that do not apply should be explicit (`N/A`), not silently omitted when omission would make the result ambiguous.

## Mutation/safety fields

For setup, uninstall, or external-config goals, report:

```text
UNRELATED_DRIFT_TOUCHED=<true|false>
EXTERNAL_CONFIG_MUTATED=<true|false>
OWNERSHIP_PROOF=<PASS|FAIL|UNPROVEN|N/A>
SENSITIVE_OUTPUT=<true|false>
```

## Visual evidence bundle

Once visual CI exists, retain at least:

- full-window context screenshot;
- cropped target-tab screenshots for each asserted state;
- animation frame set or derived animation evidence;
- UI Automation dump for the target tab;
- machine assertion summary;
- exact candidate SHA and runner identity.

Suggested artifact layout:

```text
artifacts/
  manifest.json
  uia.json
  assertions.json
  ready.png
  working-001.png
  working-002.png
  result-ready.png
  approval.png
  warning.png
  interrupted.png
  failed.png
  reset.png
  animation.gif
```

## Evidence rule

Absence of a failure is not proof of success. If the required observation did not execute, the disposition is `BLOCKED` or `UNPROVEN`.
