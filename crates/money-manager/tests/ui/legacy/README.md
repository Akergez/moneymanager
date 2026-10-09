`staging.rdx` is a pre-accounts (four-collection) binary RDX fixture. It holds
one Food category and one unsynced expense of 12.50 dated 2026-10-01, stamped
with source 42 and time 64. It uses the previous GUI's staging format.

The upgrade scenario copies it into the old ledger directory, starts with
legacy source/remote settings, converts it through the GUI, and checks that
the workspace contains the expense, the remote is reset, and Settings opens.
The runner also checks that the original staging bytes remain in the backup
and that old transport keys have left settings.json. Core tests separately
cover sealed chunks, tombstones, corrupted input and retry after conversion.
