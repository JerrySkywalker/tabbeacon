# Lab evidence index

Raw isolated output lives under the ignored
`target/g05r-lab/<family>/<run-key>` tree. This tracked index binds the final
selected summaries without committing owner configuration or transient homes.

| Family | Run key | Summary SHA-256 | Result |
| --- | --- | --- | --- |
| compatibility | `20260815-004718` | `4fc467693e2de06b3c8fe61a62d066f6401ebafe0f42817e7c9d1b5242dbb454` | declared matrix PASS |
| trust-forensics | `20260815-004504` | `0497a75580180d63930c0a275322b5dbd39aabec74f691101c9bdbe114870803` | PASS after P1 fix |
| windows-quoting | `20260815-004240` | `337ab7737f44f0dcf4aa464f98d14eb9ae74af5a0aec3e49291f95d6c71d37af` | supported/rejected cases PASS; one environment boundary |
| config-chaos | `20260815-004738` | `73107a2d04f2024fe7f18bce035545cd831f89ac7028617220bff608de7949a1` | PASS |
| hook-fuzz | `20260815-004738` | `e041ebfde9a276468c1bcc632ec0c26e9412a4da8d8d74df06cd6f9cc7e18ecf` | PASS |

The tracked PowerShell harnesses reproduce each family. Final Rust gate results,
hardening commit, remote branch, production freeze proof, and directory digest
are recorded in the final receipt.
