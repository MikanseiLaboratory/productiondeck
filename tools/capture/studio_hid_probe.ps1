# Studio HID feature probe.
#
# Opens the Stream Deck Studio HID interface through the Windows HID API,
# prints HidP_GetCaps (input/output/feature report lengths) and reads every
# requested feature report with HidD_GetFeature.
#
# Example:
#   pwsh -File tools/capture/studio_hid_probe.ps1 -ReportIds 0..63

param(
  [ushort]$Vid = 0x0fd9,
  [ushort]$ProductId = 0x00aa,
  [int[]]$ReportIds = 0x03..0x1f
)

$source = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class StudioHidProbe {
    [DllImport("hid.dll")] public static extern void HidD_GetHidGuid(out Guid guid);
    [DllImport("setupapi.dll", CharSet=CharSet.Auto, SetLastError=true)] public static extern IntPtr SetupDiGetClassDevs(ref Guid g, IntPtr e, IntPtr h, uint f);
    [StructLayout(LayoutKind.Sequential)] public struct DID { public uint cbSize; public Guid guid; public uint flags; public IntPtr reserved; }
    [DllImport("setupapi.dll", SetLastError=true)] public static extern bool SetupDiEnumDeviceInterfaces(IntPtr set, IntPtr info, ref Guid g, uint index, ref DID data);
    [DllImport("setupapi.dll", CharSet=CharSet.Auto, SetLastError=true)] public static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr set, ref DID data, IntPtr detail, uint size, out uint required, IntPtr info);
    [DllImport("setupapi.dll")] public static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);
    [DllImport("kernel32.dll", CharSet=CharSet.Auto, SetLastError=true)] public static extern IntPtr CreateFile(string name, uint access, uint share, IntPtr sa, uint disp, uint flags, IntPtr tmpl);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
    [StructLayout(LayoutKind.Sequential)] public struct ATTR { public uint Size; public ushort Vid; public ushort Pid; public ushort Version; }
    [DllImport("hid.dll")] public static extern bool HidD_GetAttributes(IntPtr h, ref ATTR a);
    [DllImport("hid.dll")] public static extern bool HidD_GetPreparsedData(IntPtr h, out IntPtr p);
    [DllImport("hid.dll")] public static extern bool HidD_FreePreparsedData(IntPtr p);
    [StructLayout(LayoutKind.Sequential)] public struct CAPS {
        public ushort Usage; public ushort UsagePage;
        public ushort InLen; public ushort OutLen; public ushort FeatLen;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst=17)] public ushort[] Reserved;
        public ushort Nodes;
        public ushort InBtnCaps; public ushort InValCaps; public ushort InDataIndices;
        public ushort OutBtnCaps; public ushort OutValCaps; public ushort OutDataIndices;
        public ushort FeatBtnCaps; public ushort FeatValCaps; public ushort FeatDataIndices;
    }
    [DllImport("hid.dll")] public static extern int HidP_GetCaps(IntPtr p, ref CAPS c);
    [DllImport("hid.dll")] public static extern bool HidD_GetFeature(IntPtr h, byte[] b, uint len);
    [DllImport("hid.dll")] public static extern bool HidD_GetProductString(IntPtr h, byte[] b, uint len);
    [DllImport("hid.dll")] public static extern bool HidD_GetSerialNumberString(IntPtr h, byte[] b, uint len);

    static string Z(byte[] b) { string s = Encoding.Unicode.GetString(b); int i = s.IndexOf('\0'); return i >= 0 ? s.Substring(0, i) : s; }

    public static void Run(ushort vid, ushort pid, int[] ids) {
        Guid g; HidD_GetHidGuid(out g);
        IntPtr set = SetupDiGetClassDevs(ref g, IntPtr.Zero, IntPtr.Zero, 0x12);
        var did = new DID(); did.cbSize = (uint)Marshal.SizeOf(did);
        uint index = 0;
        while (SetupDiEnumDeviceInterfaces(set, IntPtr.Zero, ref g, index++, ref did)) {
            uint required = 0; SetupDiGetDeviceInterfaceDetail(set, ref did, IntPtr.Zero, 0, out required, IntPtr.Zero);
            IntPtr detail = Marshal.AllocHGlobal((int)required);
            try {
                Marshal.WriteInt32(detail, IntPtr.Size == 8 ? 8 : 5);
                if (!SetupDiGetDeviceInterfaceDetail(set, ref did, detail, required, out required, IntPtr.Zero)) continue;
                string path = Marshal.PtrToStringAuto(IntPtr.Add(detail, 4));
                IntPtr h = CreateFile(path, 0x80000000u | 0x40000000u, 3u, IntPtr.Zero, 3u, 0u, IntPtr.Zero);
                if (h == new IntPtr(-1)) continue;
                try {
                    var a = new ATTR(); a.Size = (uint)Marshal.SizeOf(a);
                    if (!HidD_GetAttributes(h, ref a) || a.Vid != vid || a.Pid != pid) continue;
                    var caps = new CAPS(); IntPtr prep;
                    if (HidD_GetPreparsedData(h, out prep)) { HidP_GetCaps(prep, ref caps); HidD_FreePreparsedData(prep); }
                    var s = new byte[256]; HidD_GetProductString(h, s, (uint)s.Length);
                    var sn = new byte[256]; HidD_GetSerialNumberString(h, sn, (uint)sn.Length);
                    Console.WriteLine($"PATH {path}");
                    Console.WriteLine($"VID:PID {a.Vid:x4}:{a.Pid:x4} bcd={a.Version:x4} product='{Z(s)}' serial='{Z(sn)}'");
                    Console.WriteLine($"CAPS in={caps.InLen} out={caps.OutLen} feature={caps.FeatLen} featbtn={caps.FeatBtnCaps}");
                    foreach (int id in ids) {
                        var buf = new byte[Math.Max(64, (int)caps.FeatLen)];
                        buf[0] = (byte)id;
                        bool ok = HidD_GetFeature(h, buf, (uint)buf.Length);
                        int err = Marshal.GetLastWin32Error();
                        var hex = new StringBuilder();
                        for (int i = 0; i < Math.Min(buf.Length, 32); i++) hex.Append(buf[i].ToString("x2"));
                        Console.WriteLine($"feature {id:x2} ok={ok} err={err} {hex}");
                    }
                } finally { CloseHandle(h); }
            } finally { Marshal.FreeHGlobal(detail); }
        }
        SetupDiDestroyDeviceInfoList(set);
    }
}
'@

Add-Type -TypeDefinition $source -Language CSharp -ErrorAction Stop
[StudioHidProbe]::Run($Vid, $ProductId, $ReportIds)

