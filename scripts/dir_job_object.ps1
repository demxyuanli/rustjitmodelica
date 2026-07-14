# Job object P/Invoke for run_modelica_dir_regression.ps1 (process memory cap + KILL_ON_JOB_CLOSE).
# Dot-source from repository root scripts path.

$script:DirJobInterop = $null

if ($env:OS -ne "Windows_NT") { return }

try {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class DirJobNative
{
    public const int JobObjectExtendedLimitInformation = 9;
    public const uint JobObjectLimitKillOnJobClose  = 0x2000u;
    public const uint JobObjectLimitProcessMemory  = 0x0100u;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateJobObject(IntPtr lpJobAttributes, string lpName);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetInformationJobObject(IntPtr hJob, int JobObjectInfoClass, ref JOBOBJECT_EXTENDED_LIMIT_INFORMATION lp, uint cb);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool AssignProcessToJobObject(IntPtr hJob, IntPtr hProcess);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr h);

    [StructLayout(LayoutKind.Sequential)]
    public struct IO_COUNTERS
    {
        public ulong a, b, c, d, e, f;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public uint _pad0;
        public UIntPtr MinWorking;
        public UIntPtr MaxWorking;
        public uint Active;
        public uint _pad1;
        public UIntPtr Affinity;
        public uint Pri;
        public uint Sch;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION Basic;
        public IO_COUNTERS Io;
        public UIntPtr ProcMem;
        public UIntPtr JobMem;
        public UIntPtr PeakProc;
        public UIntPtr PeakJob;
    }

    public static int GetExtendedSize() { return Marshal.SizeOf(typeof(JOBOBJECT_EXTENDED_LIMIT_INFORMATION)); }

    public static bool SetLimits(IntPtr job, int perProcessMemoryLimitMb)
    {
        var x = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        if (perProcessMemoryLimitMb > 0)
        {
            x.Basic.LimitFlags = JobObjectLimitKillOnJobClose | JobObjectLimitProcessMemory;
            ulong b = (ulong)perProcessMemoryLimitMb * 1024ul * 1024ul;
            x.ProcMem = new UIntPtr(b);
        }
        else
        {
            x.Basic.LimitFlags = JobObjectLimitKillOnJobClose;
        }
        return SetInformationJobObject(job, JobObjectExtendedLimitInformation, ref x, (uint)GetExtendedSize());
    }
}
"@
    $script:DirJobInterop = "OK"
} catch {
    $script:DirJobInterop = $null
    Write-Warning ("dir_job_object: Add-Type failed: " + $_.Exception.Message)
}

function New-DirRegressionJob { return [IntPtr] [DirJobNative]::CreateJobObject([IntPtr]::Zero, $null) }

function Set-DirRegressionJobLimits {
    param(
        [IntPtr]$Job,
        [int]$PerProcessMemoryLimitMb
    )
    if ($Job -eq [IntPtr]::Zero) { return $false }
    if ($null -eq $script:DirJobInterop) { return $false }
    return [bool][DirJobNative]::SetLimits($Job, $PerProcessMemoryLimitMb)
}

function Add-ProcessToDirRegressionJob {
    param(
        [IntPtr]$Job,
        [IntPtr]$ProcessHandle
    )
    if ($Job -eq [IntPtr]::Zero) { return $true }
    if ($null -eq $script:DirJobInterop) { return $true }
    return [bool][DirJobNative]::AssignProcessToJobObject($Job, $ProcessHandle)
}

function Close-DirRegressionJob {
    param([IntPtr]$Job)
    if ($null -eq $script:DirJobInterop) { return }
    if ($Job -ne [IntPtr]::Zero) { [void][DirJobNative]::CloseHandle($Job) }
}
