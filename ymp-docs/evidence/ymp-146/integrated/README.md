# Integrated recovery and accepted UI verification

The single combined-source sequence passed on macOS arm64: formatting, strict
Clippy and workspace tests (627 passed, zero failures, two existing ignored).
[manifest.json](manifest.json) records commands, times, source and toolchain
bindings, test totals and explicit limits. The parent verified each retained log
against its recorded SHA-256 and retained the runner unchanged. Raw logs are not
reformatted. [retention.json](retention.json) binds every copied artifact.

The reviewer compared 218 execution inputs and preserved the unrelated dirty
request document. Only documentation/acceptance commits followed source c28f501
during the checks. [executables.json](executables.json) records the 32 produced
executables; binaries themselves are retained in the external target directory.
These checks cover accepted backend743543f plus accepted UI149, before the new
recovery UI currently being implemented in its separate fork.

No Linux execution, real model inference, live-session continuation, release
installation or comparative trial occurred. The installed executable remains
0.4.6. All commands finished; the next delivery step is recovery UI completion
and its independent acceptance, not another identical integration test cycle.
