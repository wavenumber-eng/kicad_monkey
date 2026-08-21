"""Small dependency-free child-process peak-memory sampler for Rack tests."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Final


_SAMPLE_INTERVAL_SECONDS: Final = 0.005


class _ProcessMemoryCounters(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def _windows_peak_rss(pid: int) -> int | None:
    query_information = 0x0400
    virtual_memory_read = 0x0010
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL
    psapi.GetProcessMemoryInfo.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(_ProcessMemoryCounters),
        wintypes.DWORD,
    ]
    psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
    handle = kernel32.OpenProcess(query_information | virtual_memory_read, False, pid)
    if not handle:
        return None
    try:
        counters = _ProcessMemoryCounters()
        counters.cb = ctypes.sizeof(counters)
        ok = psapi.GetProcessMemoryInfo(
            handle,
            ctypes.byref(counters),
            counters.cb,
        )
        return int(counters.PeakWorkingSetSize) if ok else None
    finally:
        kernel32.CloseHandle(handle)


def _linux_peak_rss(pid: int) -> int | None:
    try:
        status = (Path("/proc") / str(pid) / "status").read_text(encoding="ascii")
    except OSError:
        return None
    for line in status.splitlines():
        if line.startswith("VmHWM:"):
            return int(line.split()[1]) * 1024
    return None


def _posix_current_rss(pid: int) -> int | None:
    try:
        completed = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(pid)],
            capture_output=True,
            text=True,
            timeout=1,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    value = completed.stdout.strip()
    return int(value) * 1024 if value.isdecimal() else None


def _peak_rss(pid: int) -> int | None:
    if os.name == "nt":
        return _windows_peak_rss(pid)
    if sys.platform.startswith("linux"):
        return _linux_peak_rss(pid)
    return _posix_current_rss(pid)


def run_with_peak_rss(
    command: list[str],
    *,
    cwd: Path,
    timeout: float,
) -> tuple[subprocess.CompletedProcess[str], int]:
    """Run one command and return its captured result plus sampled peak RSS."""
    process = subprocess.Popen(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    peak_rss = 0
    deadline = time.monotonic() + timeout
    while process.poll() is None:
        sample = _peak_rss(process.pid)
        if sample is not None:
            peak_rss = max(peak_rss, sample)
        if time.monotonic() >= deadline:
            process.kill()
            stdout, stderr = process.communicate()
            raise subprocess.TimeoutExpired(command, timeout, stdout, stderr)
        time.sleep(_SAMPLE_INTERVAL_SECONDS)

    final_sample = _peak_rss(process.pid)
    if final_sample is not None:
        peak_rss = max(peak_rss, final_sample)
    stdout, stderr = process.communicate()
    completed = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    return completed, peak_rss
