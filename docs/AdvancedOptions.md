# Advanced ProcDump Options

These options are available behind the **Advanced Options** toggle on the
ProcDump configuration page. They cover specialised monitoring scenarios
that most users will not need for routine support workflows.

---

## Performance Counters (`-p`, `-pl`)

| Flag | Description |
|------|-------------|
| `-p <counter\threshold>` | Create a dump when a Windows performance counter **exceeds** the threshold. |
| `-pl <counter\threshold>` | Create a dump when a Windows performance counter **drops below** the threshold. |

**Format:** `\Category(Instance)\Counter\Threshold`

**Examples:**

```
\Process(myapp)\Handle Count\10000
\Process(myapp)\Thread Count\500
\.NET CLR Memory(myapp)\# Bytes in all Heaps\2000000000
```

> **Tip:** Run `typeperf -q` in a terminal to list available counters.

---

## Exception Filtering (`-f`, `-fx`)

| Flag | Description |
|------|-------------|
| `-f <filter,...>` | Only dump if the exception message **contains** one of the substrings. |
| `-fx <filter,...>` | Skip the dump if the exception message **contains** any substring. |

Use comma-separated values for multiple filters.

**Examples:**

- `-f "OutOfMemory,StackOverflow"` — only capture OOM or stack overflow crashes.
- `-fx "ThreadAbort"` — skip the dump for routine thread-abort exceptions.

Filters are case-insensitive substring matches on the full exception description.

---

## WER Integration (`-wer`)

| Flag | Description |
|------|-------------|
| `-wer` | Register ProcDump as the Windows Error Reporting (WER) just-in-time debugger. |

Use this when the standard exception trigger (`-e`) does not fire because
WER handles the crash first. Registering ProcDump as the WER debugger lets
it intercept the crash before WER takes over.

> **Note:** `-wer` modifies a machine-wide registry key and may require
> Administrator privileges. It replaces any existing JIT debugger registration.

---

## Avoid-Terminate Timeout (`-at`)

| Flag | Description |
|------|-------------|
| `-at <seconds>` | Cancel a dump-in-progress if it takes longer than the specified duration. |

Use this when the monitored process has a **service-control timeout** and
must shut down within a deadline. Without `-at`, a very large dump write
could block process termination indefinitely.

Recommended value: slightly shorter than the service-control timeout
(default 30 seconds on Windows).

---

## When *Not* to Use Advanced Options

- **Performance counters** require the counter to exist on the target machine.
  Missing counters cause ProcDump to exit immediately.
- **Exception filters** only apply when `-e` (exception trigger) is active.
  They have no effect with CPU, memory, or hang triggers.
- **WER registration** is machine-wide and persists across reboots until
  explicitly removed. Do not enable unless you understand the impact.

For most support scenarios the built-in **Scenario presets** on the main
ProcDump page are sufficient. See [ProcDumpQuickStart.md](ProcDumpQuickStart.md).
