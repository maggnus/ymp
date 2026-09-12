# Long metadata paths and internal team transport

YMP-128 corrects a reproducible release-preflight failure. The real installer
completed, but its offline demo failed with `path must be shorter than SUN_LEN`
when the application home was inside the system's long temporary directory.
Changing the test to use a shorter home would hide the failure.

## Change

The socket file remains under the selected metadata home's `run` directory. Its
normal address is absolute so native children can connect from their own working
directories. If the operating system cannot represent that address, a private
directory under `/tmp` contains a symlink to the metadata `run` directory. The
short alias is the transport address; it contains no conversation, credentials or
source files. Its directory has mode `0700`; the actual socket has mode `0600`.
The process working directory never changes.

The binding owns its actual filesystem entry and optional temporary alias.
Shutdown removes the socket only while its device/inode still matches the one
created by that binding. A failed bind cannot delete an existing listener, and an
old binding cannot unlink a replacement listener. The temporary directory removes
only the alias. Existing assignment capabilities, grant revocation, request
validation and public stdio framing remain unchanged.

This is normal-shutdown and setup-failure cleanup. Abrupt process termination can
leave stale transport entries, as it could before for socket files; this change
does not introduce a crash-recovery sweep or promise cleanup after `SIGKILL`.

## Discriminating evidence

| Check | Result |
| --- | --- |
| New real executable long-home demo on the original source | Failed with the original `SUN_LEN` error, exit 101 |
| Same regression after the correction | Passed; artifact in the selected directory, metadata retained and socket removed |
| Short address without alias | Round trip and socket permissions/cleanup passed |
| Two long addresses in one metadata directory | Private distinct aliases, both round trips, independent shutdown and retained metadata passed |
| Attempt to bind an occupied pathname | Existing listener remained usable |
| Old binding dropped after pathname replacement | Replacement listener remained usable |
| Mutation removing the inode ownership comparison | Replacement-listener test failed, exit 101; source restored byte for byte |
| Original failed installer/demo home reused with the corrected executable | Passed in the unchanged long-path setup; no artifact under metadata and no remaining socket |

Logs are `/tmp/ymp128-long-home-before.log`,
`/tmp/ymp128-long-home-after.log`, `/tmp/ymp128-socket-tests.log`,
`/tmp/ymp128-replacement-control.log` and
`/tmp/ymp128-original-install-demo-after.log`. The original installer failure is
retained at
`/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-install-preflight-v9wiwh0v/demo.log`.
An earlier preflight harness compared `/var` and `/private/var` textually; that
harness-only path comparison was corrected before the real demo failure above.

The final candidate also contains the separately accepted workspace-policy test
`d4550eb` (mapped to `97d3213`) and Opus UI follow-up `f4ad62c` (mapped to
`bc1db5d`). They are not part of the socket implementation. Required formatting,
Clippy, all **382 workspace tests**, and executable build passed on unchanged
source bytes. Commands, log hashes and source hashes are in
`/tmp/ymp128-final/` and the companion evidence ledger.

All workloads were mock/scripted; no native model request was made. Independent
review and integration remain required before this task closes. Final-version
installation still belongs to YMP-121.
