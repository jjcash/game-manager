# SPDX-License-Identifier: GPL-3.0-or-later
# Isolated API experiment, NOT an approved-root reader or a production test.
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class ReparseProbe {
    [StructLayout(LayoutKind.Sequential)] struct UnicodeString { public ushort Length, MaximumLength; public IntPtr Buffer; }
    [StructLayout(LayoutKind.Sequential)] struct ObjectAttributes { public int Length; public IntPtr RootDirectory, ObjectName; public uint Attributes; public IntPtr SecurityDescriptor, SecurityQualityOfService; }
    [StructLayout(LayoutKind.Sequential)] struct IoStatus { public IntPtr Status, Information; }
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern SafeFileHandle CreateFileW(string name, uint access, uint share, IntPtr security, uint disposition, uint flags, IntPtr template);
    [DllImport("ntdll.dll")] static extern int NtCreateFile(out SafeFileHandle file, uint access, ref ObjectAttributes attributes, out IoStatus status, IntPtr allocation, uint fileAttributes, uint share, uint disposition, uint options, IntPtr ea, uint eaLength);
    public static uint Open(string root, string relative) {
        using (var directory = CreateFileW(root, 0x80, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero)) {
            if (directory.IsInvalid) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            IntPtr buffer = Marshal.StringToHGlobalUni(relative);
            IntPtr name = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(UnicodeString)));
            try {
                var us = new UnicodeString { Length = checked((ushort)(relative.Length * 2)), MaximumLength = checked((ushort)(relative.Length * 2)), Buffer = buffer };
                Marshal.StructureToPtr(us, name, false);
                var oa = new ObjectAttributes { Length = Marshal.SizeOf(typeof(ObjectAttributes)), RootDirectory = directory.DangerousGetHandle(), ObjectName = name, Attributes = 0x1000 };
                IoStatus io;
                SafeFileHandle result;
                int status = NtCreateFile(out result, 0x00100081, ref oa, out io, IntPtr.Zero, 0, 7, 1, 0x20, IntPtr.Zero, 0);
                if (result != null) result.Dispose();
                return unchecked((uint)status);
            } finally { Marshal.FreeHGlobal(name); Marshal.FreeHGlobal(buffer); }
        }
    }
}
'@
$base = Join-Path ([IO.Path]::GetTempPath()) ('gm-m13a-' + [Guid]::NewGuid().ToString('N'))
$root = Join-Path $base 'root'
$outside = Join-Path $base 'outside'
try {
    [IO.Directory]::CreateDirectory($root) | Out-Null
    [IO.Directory]::CreateDirectory($outside) | Out-Null
    [IO.File]::WriteAllText((Join-Path $root 'ok.txt'), 'inside')
    [IO.File]::WriteAllText((Join-Path $outside 'sentinel.txt'), 'outside')
    New-Item -ItemType Junction -Path (Join-Path $root 'junction') -Target $outside | Out-Null
    New-Item -ItemType HardLink -Path (Join-Path $root 'hardlink.txt') -Target (Join-Path $outside 'sentinel.txt') | Out-Null
    $symlinkAvailable = $true
    try { New-Item -ItemType SymbolicLink -Path (Join-Path $root 'symlink.txt') -Target (Join-Path $outside 'sentinel.txt') -ErrorAction Stop | Out-Null }
    catch { $symlinkAvailable = $false }
    # No sharing ensures a redirected content open would fail with a sharing
    # violation instead of the expected reparse refusal.
    $locked = [IO.File]::Open((Join-Path $outside 'sentinel.txt'), 'Open', 'ReadWrite', 'None')
    try {
        $ordinary = [ReparseProbe]::Open($root, 'ok.txt')
        $junction = [ReparseProbe]::Open($root, 'junction\sentinel.txt')
        $hardlink = [ReparseProbe]::Open($root, 'hardlink.txt')
        if ($ordinary -ne 0 -or $junction -ne [uint32]3221226763 -or $hardlink -ne [uint32]3221225539) { throw ('Unexpected NTSTATUS: ordinary={0:X8}, junction={1:X8}, hardlink={2:X8}' -f $ordinary, $junction, $hardlink) }
        'ordinary: STATUS_SUCCESS; intermediate junction: STATUS_REPARSE_POINT_ENCOUNTERED; sentinel held without sharing'
        'hard link: STATUS_SHARING_VIOLATION; no-reparse alone attempted to open the external sentinel object'
        if ($symlinkAvailable) {
            $symlink = [ReparseProbe]::Open($root, 'symlink.txt')
            if ($symlink -ne [uint32]3221226763) { throw ('Unexpected symlink NTSTATUS: {0:X8}' -f $symlink) }
            'leaf symbolic link: STATUS_REPARSE_POINT_ENCOUNTERED'
        } else { 'leaf symbolic link: fixture unavailable' }
    } finally { $locked.Dispose() }
} finally {
    # Only remove the uniquely generated, known temporary directory.
    if (Test-Path -LiteralPath $base) { Remove-Item -LiteralPath $base -Recurse -Force }
}
