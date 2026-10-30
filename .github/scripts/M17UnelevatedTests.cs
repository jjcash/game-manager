// SPDX-License-Identifier: GPL-3.0-or-later
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
using System.Threading.Tasks;

// CI-only, fixed-purpose feasibility runner. No arbitrary executable/command input.
// This restricts our own token; it is not a sandbox for hostile repository code.
static class M17UnelevatedTests
{
    [StructLayout(LayoutKind.Sequential)] struct SidAttributes { public IntPtr Sid; public uint Attributes; }
    [StructLayout(LayoutKind.Sequential)] struct Groups { public uint Count; public SidAttributes First; }
    [StructLayout(LayoutKind.Sequential)] struct Luid { public uint Low; public int High; }
    [StructLayout(LayoutKind.Sequential)] struct Privilege { public Luid Id; public uint Attributes; }
    [StructLayout(LayoutKind.Sequential)] struct Privileges { public uint Count; public Privilege First; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] struct Startup {
        public uint cb; public string reserved, desktop, title;
        public uint x, y, width, height, charsX, charsY, fill, flags;
        public ushort show, reservedSize; public IntPtr reservedBytes, stdin, stdout, stderr;
    }
    [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr process, thread; public uint pid, tid; }
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr h);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool OpenProcessToken(IntPtr p, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool GetTokenInformation(IntPtr t, int kind, IntPtr data, uint size, out uint needed);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool CreateRestrictedToken(IntPtr t, uint flags, uint disabledCount, [In] SidAttributes[] disabled, uint deletedCount, IntPtr deleted, uint restrictedCount, IntPtr restricted, out IntPtr result);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool SetTokenInformation(IntPtr t, int kind, IntPtr data, uint size);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool LookupPrivilegeNameW(string system, ref Luid id, StringBuilder name, ref uint size);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool CreateProcessAsUserW(IntPtr token, string app, StringBuilder line, IntPtr processAttributes, IntPtr threadAttributes, bool inherit, uint flags, IntPtr environment, string directory, ref Startup startup, out ProcessInfo process);
    [DllImport("kernel32.dll", SetLastError = true)] static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll", SetLastError = true)] static extern uint WaitForSingleObject(IntPtr h, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetExitCodeProcess(IntPtr process, out uint code);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool TerminateProcess(IntPtr process, uint code);
    [DllImport("kernel32.dll", SetLastError = true)] static extern uint GetConsoleProcessList([Out] uint[] processes, uint count);

    static void Check(bool ok, string operation) {
        if (!ok) throw new Win32Exception(Marshal.GetLastWin32Error(), operation);
    }
    static void ProbeConsole(string label) {
        // Read-only observation, not a token/security assertion. A one-entry
        // buffer suffices to report the count without logging process IDs.
        uint count = GetConsoleProcessList(new uint[1], 1);
        int error = count == 0 ? Marshal.GetLastWin32Error() : 0;
        Console.WriteLine("{0}: console process count={1}; console query error={2}", label, count, error);
    }
    static IntPtr Info(IntPtr token, int kind) {
        uint size;
        bool first = GetTokenInformation(token, kind, IntPtr.Zero, 0, out size);
        int error = Marshal.GetLastWin32Error();
        if (first || (error != 122 && error != 24) || size == 0 || size > 1048576)
            throw new InvalidOperationException("Invalid token query size: kind=" + kind + "; first=" + first + "; error=" + Marshal.GetLastWin32Error() + "; bytes=" + size);
        IntPtr data = Marshal.AllocHGlobal((int)size);
        try { Check(GetTokenInformation(token, kind, data, size, out size), "GetTokenInformation " + kind); return data; }
        catch { Marshal.FreeHGlobal(data); throw; }
    }
    static int Scalar(IntPtr token, int kind) {
        IntPtr data = Info(token, kind);
        try { return Marshal.ReadInt32(data); } finally { Marshal.FreeHGlobal(data); }
    }
    static uint Group(IntPtr token, string sid) {
        IntPtr data = Info(token, 2);
        try {
            uint count = (uint)Marshal.ReadInt32(data);
            int offset = (int)Marshal.OffsetOf(typeof(Groups), "First");
            int stride = Marshal.SizeOf(typeof(SidAttributes));
            if (count > 1024) throw new InvalidOperationException("Excessive token group count");
            for (int i = 0; i < count; i++) {
                SidAttributes group = (SidAttributes)Marshal.PtrToStructure(IntPtr.Add(data, offset + i * stride), typeof(SidAttributes));
                if (new SecurityIdentifier(group.Sid).Value == sid) return group.Attributes;
            }
            return 0;
        } finally { Marshal.FreeHGlobal(data); }
    }
    static void Snapshot(IntPtr token, string label, bool requireSafe) {
        int elevated = Scalar(token, 20), type = Scalar(token, 8);
        IntPtr data = Info(token, 25);
        string integrity;
        try { integrity = new SecurityIdentifier(Marshal.ReadIntPtr(data)).Value; }
        finally { Marshal.FreeHGlobal(data); }
        uint admin = Group(token, "S-1-5-32-544"), localAdmin = Group(token, "S-1-5-114");
        Console.WriteLine("{0}: TokenElevation={1}; TokenType={2}; Integrity={3}; Administrators=0x{4:X}; LocalAccountAndAdministrator=0x{5:X}", label, elevated, type, integrity, admin, localAdmin);
        bool unsafePrivilege = false;
        data = Info(token, 3);
        try {
            uint count = (uint)Marshal.ReadInt32(data);
            if (count > 1024) throw new InvalidOperationException("Excessive privilege count");
            int offset = (int)Marshal.OffsetOf(typeof(Privileges), "First"), stride = Marshal.SizeOf(typeof(Privilege));
            for (int i = 0; i < count; i++) {
                Privilege privilege = (Privilege)Marshal.PtrToStructure(IntPtr.Add(data, offset + stride * i), typeof(Privilege));
                StringBuilder name = new StringBuilder(256); uint capacity = 256;
                Check(LookupPrivilegeNameW(null, ref privilege.Id, name, ref capacity), "LookupPrivilegeNameW");
                bool enabled = (privilege.Attributes & 2) != 0;
                Console.WriteLine("{0}: privilege {1} enabled={2}", label, name, enabled);
                unsafePrivilege |= enabled && name.ToString() != "SeChangeNotifyPrivilege";
            }
        } finally { Marshal.FreeHGlobal(data); }
        if (requireSafe && (unsafePrivilege || elevated != 0 || type != 1 || integrity != "S-1-16-8192" ||
            (admin != 0 && ((admin & 6) != 0 || (admin & 16) == 0)) ||
            (localAdmin != 0 && ((localAdmin & 6) != 0 || (localAdmin & 16) == 0))))
            throw new InvalidOperationException("Required non-elevated primary token not established");
    }
    static string Quote(string value) {
        StringBuilder result = new StringBuilder("\""); int slashes = 0;
        foreach (char c in value) {
            if (c == '\\') { slashes++; continue; }
            result.Append('\\', c == '"' ? slashes * 2 + 1 : slashes);
            result.Append(c); slashes = 0;
        }
        result.Append('\\', slashes * 2); return result.Append('"').ToString();
    }
    static string Tool(string name) {
        foreach (string item in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(';')) {
            if (String.IsNullOrWhiteSpace(item)) continue;
            string path = Path.Combine(item.Trim().Trim('"'), name);
            if (Path.IsPathRooted(path) && File.Exists(path)) return Path.GetFullPath(path);
        }
        throw new FileNotFoundException("Required tool missing: " + name);
    }
    sealed class Output {
        public int Exit; public string Stdout, Stderr;
    }
    sealed class ChildFailure : Exception {
        public readonly int Exit;
        public ChildFailure(string stage, int code) : base(stage + " failed, exit=" + code) { Exit = code; }
    }
    // No process-tree kill or kill-on-close job: an authorized synthetic child
    // must survive its launcher parent. Deadlines cover only handles we own.
    const int CleanupMilliseconds = 30000;
    static int Deadline(string phase) {
        switch (phase) {
            case "probe": return 30000;
            case "prerequisites": return 8 * 60000;
            case "native": return 15 * 60000;
            case "suite": return 20 * 60000;
            default: throw new ArgumentException("Unknown fixed phase");
        }
    }
    static async Task Drain(StreamReader reader, StringBuilder text, TextWriter echo) {
        char[] buffer = new char[4096];
        int read;
        while ((read = await reader.ReadAsync(buffer, 0, buffer.Length)) != 0) {
            lock (text) { text.Append(buffer, 0, read); }
            if (echo != null) { echo.Write(buffer, 0, read); echo.Flush(); }
        }
    }
    static string Captured(StringBuilder text) { lock (text) { return text.ToString(); } }
    static Output Command(string app, string args, string directory, string stage, int milliseconds, bool echo = true) {
        Process process = new Process();
        bool started = false, exited = false;
        StringBuilder stdout = new StringBuilder(), stderr = new StringBuilder();
        Task[] drains = null;
        try {
            process.StartInfo = new ProcessStartInfo(app, args) {
                WorkingDirectory = directory, UseShellExecute = false, CreateNoWindow = true,
                RedirectStandardInput = true, RedirectStandardOutput = true, RedirectStandardError = true
            };
            if (!process.Start()) throw new InvalidOperationException("Command did not start");
            started = true;
            process.StandardInput.Close();
            drains = new[] { Drain(process.StandardOutput, stdout, echo ? Console.Out : null), Drain(process.StandardError, stderr, echo ? Console.Error : null) };
            exited = process.WaitForExit(milliseconds);
            if (!exited) {
                Console.Error.WriteLine("TIMEOUT: {0} after {1} ms; terminating only owned PID {2}", stage, milliseconds, process.Id);
                process.Kill();
                exited = process.WaitForExit(CleanupMilliseconds);
                if (!exited) throw new InvalidOperationException("Cleanup termination not confirmed: " + stage);
                // Synthetic timeout result; ordinary exits below remain exact.
                return new Output { Exit = 124, Stdout = Captured(stdout), Stderr = Captured(stderr) };
            }
            Console.WriteLine("{0} command exit code: {1}", stage, process.ExitCode);
            try {
                if (!Task.WaitAll(drains, CleanupMilliseconds))
                    throw new TimeoutException("Output collection timed out: " + stage + "; descendants may still own pipe writers");
            } catch (Exception outputFailure) {
                Console.Error.WriteLine("OUTPUT FAILURE ({0}; original exit {1}): {2}", stage, process.ExitCode, outputFailure);
                if (process.ExitCode != 0) throw new ChildFailure(stage + " output collection", process.ExitCode);
                throw;
            }
            return new Output { Exit = process.ExitCode, Stdout = Captured(stdout), Stderr = Captured(stderr) };
        } finally {
            if (started && !exited) {
                try {
                    if (!process.HasExited) process.Kill();
                    exited = process.WaitForExit(CleanupMilliseconds);
                    if (!exited) Console.Error.WriteLine("CLEANUP FAILURE: termination not confirmed: " + stage);
                } catch (Exception cleanup) { Console.Error.WriteLine("CLEANUP FAILURE ({0}): {1}", stage, cleanup); }
            }
            if (drains != null) {
                try {
                    if (!Task.WaitAll(drains, CleanupMilliseconds))
                        Console.Error.WriteLine("CLEANUP FAILURE: output EOF not confirmed: " + stage);
                } catch (Exception cleanup) { Console.Error.WriteLine("CLEANUP FAILURE ({0} output): {1}", stage, cleanup); }
            }
            Console.Out.Flush(); Console.Error.Flush();
            if (!started || exited) process.Dispose();
            else Console.Error.WriteLine("CLEANUP FAILURE: retaining managed process handle until helper exit: " + stage);
        }
    }
    static void Success(Output output, string stage) {
        if (output.Exit != 0) throw new ChildFailure(stage, output.Exit);
    }
    static void Prerequisites(string root) {
        string directory = Path.Combine(root, "fixture space"), cwd = Environment.CurrentDirectory;
        Directory.CreateDirectory(directory);
        string source = Path.Combine(cwd, "src-tauri", "tests", "fixtures", "standalone", "child.rs");
        string fixture = Path.Combine(directory, "fixture.exe"), rustc = Tool("rustc.exe");
        Success(Command(rustc, "--version", cwd, "rustc prerequisite", 30000), "rustc prerequisite");
        Success(Command(Tool("cargo.exe"), "--version", cwd, "cargo prerequisite", 30000), "cargo prerequisite");
        // Same source and MSVC manifest switches as the real native harness.
        Success(Command(rustc, "--edition=2024 -Dwarnings " + Quote(source) + " -o " + Quote(fixture), cwd, "Rust/MSVC fixture compilation", 180000), "Rust/MSVC fixture compilation");
        string manifest = Path.Combine(directory, "administrator.manifest");
        File.WriteAllText(manifest, "<assembly xmlns=\"urn:schemas-microsoft-com:asm.v1\" manifestVersion=\"1.0\"><trustInfo xmlns=\"urn:schemas-microsoft-com:asm.v3\"><security><requestedPrivileges><requestedExecutionLevel level=\"requireAdministrator\" uiAccess=\"false\"/></requestedPrivileges></security></trustInfo></assembly>");
        Success(Command(rustc, "--edition=2024 -Dwarnings " + Quote(source) + " -o " + Quote(Path.Combine(directory, "requires-administrator.exe")) + " -C link-arg=/MANIFEST:EMBED -C link-arg=/MANIFESTUAC:NO -C " + Quote("link-arg=/MANIFESTINPUT:" + manifest), cwd, "MSVC manifest compilation (not executed)", 180000), "MSVC manifest compilation (not executed)");
        File.WriteAllText(Path.Combine(directory, "fixture-control.txt"), "23 0");
        Output child = Command(fixture, Quote("literal argument"), directory, "synthetic fixture", 30000, false);
        if (child.Exit != 23 || child.Stdout != "M17_FIXTURE_STDOUT_MUST_NOT_LEAK" || child.Stderr != "M17_FIXTURE_STDERR_MUST_NOT_LEAK" || File.ReadAllText(Path.Combine(directory, "exited.txt")) != "23")
            throw new InvalidOperationException("Fixture output/exit propagation failed");
        using (BinaryReader report = new BinaryReader(File.OpenRead(Path.Combine(directory, "received.bin")))) {
            Func<string> field = delegate { int bytes = report.ReadInt32(); if (bytes < 0 || bytes > 65536) throw new InvalidDataException(); return Encoding.UTF8.GetString(report.ReadBytes(bytes)); };
            field();
            if (!String.Equals(field(), directory, StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("Wrong fixture working directory");
            field();
            if (field() != Environment.GetEnvironmentVariable("SystemRoot")) throw new InvalidOperationException("Fixture SystemRoot inheritance mismatch");
            if (field() != "noInput" || field() != "literal argument") throw new InvalidOperationException("Fixture stdin/argv prerequisite failed");
        }
        Directory.Delete(directory, true);
        if (Directory.Exists(directory)) throw new InvalidOperationException("Fixture cleanup failed");
        Console.WriteLine("PASS: directory read/write; Rust/MSVC and manifest compilation; child creation; stdin EOF; literal argv; separate stdout/stderr; exit 23; fixture cleanup");
    }
    static int Phase(string phase, IntPtr own, string root) {
        Snapshot(own, "actual " + phase + " child", true);
        if (phase == "probe") { ProbeConsole("actual probe child"); Console.WriteLine("M17_HARMLESS_STDOUT"); Console.Error.WriteLine("M17_HARMLESS_STDERR"); return 17; }
        if (phase == "prerequisites") { Prerequisites(root); return 0; }
        string cwd = Environment.CurrentDirectory;
        if (phase == "native") {
            Output output = Command(Tool("cargo.exe"), "test --manifest-path src-tauri/Cargo.toml --all-features --locked --test standalone_launch", cwd, "native tests", Deadline("native") - 90000);
            if (output.Exit == 0 && !output.Stdout.Contains("12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"))
                throw new InvalidOperationException("Expected all 12 native tests, without exclusions");
            return output.Exit;
        }
        if (phase == "suite") {
            string pnpm = Tool("pnpm.cmd");
            if (pnpm.IndexOfAny("\"%!?&|<>^\r\n".ToCharArray()) >= 0) throw new InvalidOperationException("Unsupported pnpm command path");
            // Exactly the existing package script, including ordinary and native targets.
            return Command(Path.Combine(Environment.SystemDirectory, "cmd.exe"), "/d /s /c \"\"" + pnpm + "\" test:rust\"", cwd, "complete Rust suite", Deadline("suite") - 90000).Exit;
        }
        throw new ArgumentException("Unknown fixed phase");
    }
    static int RunPhase(IntPtr token, string phase, string root) {
        string stdout = Path.Combine(root, phase + ".stdout.txt"), stderr = Path.Combine(root, phase + ".stderr.txt");
        string app = Assembly.GetExecutingAssembly().Location;
        Startup startup = new Startup(); startup.cb = (uint)Marshal.SizeOf(typeof(Startup));
        ProcessInfo process = new ProcessInfo(); bool running = false; int? observedExit = null;
        try {
            string line = Quote(app) + " --child " + phase + " " + Quote(root);
            // Final bounded CI experiment: every fixed helper phase uses the
            // console-compatible arrangement demonstrated by the hosted probe.
            uint flags = 0x00000004U; // CREATE_SUSPENDED, without CREATE_NO_WINDOW.
            Console.WriteLine("{0}: creation flags=0x{1:X8}; environment=NULL (inherit caller); desktop=NULL; inherit handles=False; cwd={2}", phase, flags, Environment.CurrentDirectory);
            if (phase == "probe") {
                ProbeConsole("probe parent");
            }
            // NULL environment deliberately inherits the caller's complete block,
            // including Rust/MSVC configuration and any caller-injected secrets.
            // CI configures no secrets; token filtering is not environment isolation.
            Check(CreateProcessAsUserW(token, app, new StringBuilder(line), IntPtr.Zero, IntPtr.Zero, false, flags, IntPtr.Zero, Environment.CurrentDirectory, ref startup, out process), "CreateProcessAsUserW " + phase);
            running = true;
            IntPtr childToken;
            Check(OpenProcessToken(process.process, 8, out childToken), "OpenProcessToken suspended " + phase + " child");
            try { Snapshot(childToken, "actual " + phase + " child before resume", true); } finally { CloseHandle(childToken); }
            if (ResumeThread(process.thread) == uint.MaxValue) Check(false, "ResumeThread");
            uint wait = WaitForSingleObject(process.process, (uint)Deadline(phase));
            if (wait == 258) throw new TimeoutException("TIMEOUT: " + phase + " after " + Deadline(phase) + " ms");
            if (wait != 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "Child wait failed: " + phase);
            running = false;
            uint exit; Check(GetExitCodeProcess(process.process, out exit), "GetExitCodeProcess");
            observedExit = unchecked((int)exit);
            Console.WriteLine("{0} actual child exit code: {1} (0x{1:X8}; signed {2})", phase, exit, observedExit.Value);
            if (phase == "probe") {
                string probeOut = ReadLog(stdout), probeError = ReadLog(stderr);
                Console.WriteLine("probe streams: stdout exists={0}, chars={1}; stderr exists={2}, chars={3}", File.Exists(stdout), probeOut.Length, File.Exists(stderr), probeError.Length);
                if (exit != 17) throw new ChildFailure("Harmless probe expected exit 17", observedExit.Value == 0 ? 1 : observedExit.Value);
                if (!probeOut.Contains("M17_HARMLESS_STDOUT") || probeError.Trim() != "M17_HARMLESS_STDERR")
                    throw new InvalidOperationException("Harmless child stdout/stderr/exit mismatch");
            }
            return unchecked((int)exit);
        } catch (Exception error) {
            // Keep the primary failure visible even if cleanup subsequently fails.
            Console.Error.WriteLine("Phase {0} failed: {1}", phase, error);
            throw;
        } finally {
            bool cleanupFailed = false;
            try {
                if (running) {
                    // Only this directly created child; descendants are deliberately
                    // not claimed to be cleaned up. The CI job is the final boundary.
                    if (WaitForSingleObject(process.process, 0) != 0)
                        Check(TerminateProcess(process.process, 124), "Terminate owned " + phase + " child");
                    if (WaitForSingleObject(process.process, CleanupMilliseconds) != 0)
                        throw new InvalidOperationException("Termination not confirmed: " + phase);
                    running = false;
                }
            } catch (Exception cleanup) {
                cleanupFailed = true; Console.Error.WriteLine("CLEANUP FAILURE: " + cleanup);
            } finally {
                foreach (IntPtr handle in new[] { process.thread, process.process }) {
                    if (handle != IntPtr.Zero && !running && !CloseHandle(handle)) {
                        cleanupFailed = true; Console.Error.WriteLine("CLEANUP FAILURE: CloseHandle error=" + Marshal.GetLastWin32Error());
                    }
                }
                if (running) Console.Error.WriteLine("CLEANUP FAILURE: retaining native child handles until helper exit; termination unconfirmed");
            }
            // Include already flushed partial logs after timeout or failed verification.
            try { Console.Write(ReadLog(stdout)); Console.Error.Write(ReadLog(stderr)); }
            catch (Exception logs) { cleanupFailed = true; Console.Error.WriteLine("OUTPUT FAILURE: " + logs); }
            if (cleanupFailed) {
                if (observedExit.HasValue && observedExit.Value != 0)
                    throw new ChildFailure(phase + " cleanup/output", observedExit.Value);
                throw new InvalidOperationException("Phase cleanup/output failed: " + phase);
            }
        }
    }
    static string ReadLog(string path) {
        if (!File.Exists(path)) return ""; // A suspended rejected child never opens logs.
        Task<string> read = Task.Run(delegate {
            using (FileStream file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite)) {
                if (file.Length > 16 * 1024 * 1024) throw new InvalidDataException("Phase log exceeds 16 MiB: " + Path.GetFileName(path));
                using (StreamReader reader = new StreamReader(file, Encoding.UTF8)) {
                    char[] buffer = new char[16 * 1024 * 1024 + 1];
                    int length = reader.ReadBlock(buffer, 0, buffer.Length);
                    if (length == buffer.Length) throw new InvalidDataException("Phase log grew beyond limit");
                    return new string(buffer, 0, length);
                }
            }
        });
        if (!read.Wait(CleanupMilliseconds)) throw new TimeoutException("Phase log collection timed out: " + Path.GetFileName(path));
        return read.Result;
    }
    static IntPtr Sid(string text) {
        SecurityIdentifier sid = new SecurityIdentifier(text);
        byte[] bytes = new byte[sid.BinaryLength]; sid.GetBinaryForm(bytes, 0);
        IntPtr memory = Marshal.AllocHGlobal(bytes.Length); Marshal.Copy(bytes, 0, memory, bytes.Length); return memory;
    }
    static void Medium(IntPtr token) {
        SecurityIdentifier sid = new SecurityIdentifier("S-1-16-8192");
        byte[] bytes = new byte[sid.BinaryLength]; sid.GetBinaryForm(bytes, 0);
        int header = Marshal.SizeOf(typeof(SidAttributes));
        IntPtr data = Marshal.AllocHGlobal(header + bytes.Length);
        try {
            IntPtr sidAddress = IntPtr.Add(data, header); Marshal.Copy(bytes, 0, sidAddress, bytes.Length);
            Marshal.StructureToPtr(new SidAttributes { Sid = sidAddress, Attributes = 0x20 }, data, false);
            Check(SetTokenInformation(token, 25, data, (uint)(header + bytes.Length)), "SetTokenInformation medium integrity");
        } finally { Marshal.FreeHGlobal(data); }
    }
    static int Main(string[] args) {
        IntPtr own = IntPtr.Zero, restricted = IntPtr.Zero;
        try {
            Check(OpenProcessToken(GetCurrentProcess(), args.Length == 0 ? 0x8BU : 8U, out own), "OpenProcessToken");
            if (args.Length == 3 && args[0] == "--child") {
                if (args[1] != "probe" && args[1] != "prerequisites" && args[1] != "native" && args[1] != "suite")
                    throw new ArgumentException("Unknown fixed phase");
                if (!String.Equals(Path.GetFullPath(args[2]), Path.Combine(Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location), "results"), StringComparison.OrdinalIgnoreCase))
                    throw new ArgumentException("Unexpected owned result directory");
                using (StreamWriter stdout = new StreamWriter(Path.Combine(args[2], args[1] + ".stdout.txt"), false, new UTF8Encoding(false)))
                using (StreamWriter stderr = new StreamWriter(Path.Combine(args[2], args[1] + ".stderr.txt"), false, new UTF8Encoding(false))) {
                    stdout.AutoFlush = true; stderr.AutoFlush = true;
                    Console.SetOut(stdout); Console.SetError(stderr);
                    try { return Phase(args[1], own, args[2]); }
                    catch (Exception error) { Console.Error.WriteLine(error); return error is ChildFailure ? ((ChildFailure)error).Exit : error is TimeoutException ? 124 : 1; }
                }
            }
            if (args.Length != 0) throw new ArgumentException("No arbitrary command is accepted");
            Snapshot(own, "parent", false);
            IntPtr admin = Sid("S-1-5-32-544"), localAdmin = Sid("S-1-5-114");
            try {
                Check(CreateRestrictedToken(own, 5, 2, new[] { new SidAttributes { Sid = admin }, new SidAttributes { Sid = localAdmin } }, 0, IntPtr.Zero, 0, IntPtr.Zero, out restricted), "CreateRestrictedToken LUA_TOKEN | DISABLE_MAX_PRIVILEGE");
            } finally { Marshal.FreeHGlobal(admin); Marshal.FreeHGlobal(localAdmin); }
            Medium(restricted); Snapshot(restricted, "restricted candidate", true);
            string root = Path.Combine(Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location), "results");
            Directory.CreateDirectory(root);
            if (RunPhase(restricted, "probe", root) != 17) return 1;
            int code = RunPhase(restricted, "prerequisites", root);
            if (code != 0) return code;
            code = RunPhase(restricted, "native", root);
            if (code != 0) return code;
            return RunPhase(restricted, "suite", root);
        } catch (Exception error) { Console.Error.WriteLine(error); return error is ChildFailure ? ((ChildFailure)error).Exit : error is TimeoutException ? 124 : 1; }
        finally { if (restricted != IntPtr.Zero) CloseHandle(restricted); if (own != IntPtr.Zero) CloseHandle(own); }
    }
}
