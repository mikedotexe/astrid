# Preserved production preparation 01

This is an immutable archive of disabled preparation, not a deployment or a
qualified numerical restore. The 77 stdlib tests passed, but subsequent static
review found that the verifier checked the lazy LSM import before service
construction. That concrete blocker prevents real execution of this version.
The superseding review records the gap without relabeling the passing limited
test evidence. Version 02 is a separate source-only successor with fresh tests.

No live service/source/package/state changes or numerical workloads occurred.
The model pin policy changes only the existing --model value to the exact
cached snapshot on a future reviewed transition; it has not been deployed.
Metadata of 102 on-disk snapshots was inspected individually, including three
LSM-shadow histories. It is not a stable global backup or restore acceptance.

External paths in copied receipts identify their original inputs. Copies are
for review, not runnable replacements. Executables, weights, installed packages
and live history/state are not copied into this archive.
