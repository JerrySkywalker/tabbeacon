# v0.7.3 PR topology

Preferred sequence:

```text
Planning PR
  -> Train A: G99 + G100
  -> Train B: G101
  -> Release PR: G102
  -> metadata-only closeout PR
```

PR #100 may remain Train A only if it can be reconciled safely against current
main without force-push history distortion. Otherwise it is preserved as
historical evidence and a clean successor Train-A PR is created.
