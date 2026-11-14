$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace GatitoNS {
  [ComImport, Guid("F8679F50-850A-41CF-9C72-430F290290C8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IPolicyConfig {
    [PreserveSig] int GetMixFormat([MarshalAs(UnmanagedType.LPWStr)] string n, IntPtr f);
    [PreserveSig] int GetDeviceFormat([MarshalAs(UnmanagedType.LPWStr)] string n, int d, IntPtr f);
    [PreserveSig] int ResetDeviceFormat([MarshalAs(UnmanagedType.LPWStr)] string n);
    [PreserveSig] int SetDeviceFormat([MarshalAs(UnmanagedType.LPWStr)] string n, IntPtr a, IntPtr b);
    [PreserveSig] int GetProcessingPeriod([MarshalAs(UnmanagedType.LPWStr)] string n, int d, IntPtr a, IntPtr b);
    [PreserveSig] int SetProcessingPeriod([MarshalAs(UnmanagedType.LPWStr)] string n, IntPtr a);
    [PreserveSig] int GetShareMode([MarshalAs(UnmanagedType.LPWStr)] string n, IntPtr a);
    [PreserveSig] int SetShareMode([MarshalAs(UnmanagedType.LPWStr)] string n, IntPtr a);
    [PreserveSig] int GetPropertyValue([MarshalAs(UnmanagedType.LPWStr)] string n, int fx, IntPtr k, IntPtr v);
    [PreserveSig] int SetPropertyValue([MarshalAs(UnmanagedType.LPWStr)] string n, int fx, IntPtr k, IntPtr v);
    [PreserveSig] int SetDefaultEndpoint([MarshalAs(UnmanagedType.LPWStr)] string n, int role);
    [PreserveSig] int SetEndpointVisibility([MarshalAs(UnmanagedType.LPWStr)] string n, int v);
  }
  [ComImport, Guid("870AF99C-171D-4F9E-AF0D-E63DF40C2BC9")] class PolicyConfigClient {}

  [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IMMDeviceEnumerator {
    [PreserveSig] int EnumAudioEndpoints(int f, int m, out IntPtr d);
    [PreserveSig] int GetDefaultAudioEndpoint(int f, int r, out IMMDevice d);
    [PreserveSig] int GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IMMDevice d);
    [PreserveSig] int RegisterEndpointNotificationCallback(IntPtr c);
    [PreserveSig] int UnregisterEndpointNotificationCallback(IntPtr c);
  }
  [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IMMDevice {
    [PreserveSig] int Activate(ref Guid iid, int ctx, IntPtr p, out IntPtr o);
    [PreserveSig] int OpenPropertyStore(int access, out IPropertyStore s);
    [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
    [PreserveSig] int GetState(out int s);
  }
  [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IPropertyStore {
    [PreserveSig] int GetCount(out int c);
    [PreserveSig] int GetAt(int i, out PropKey k);
    [PreserveSig] int GetValue(ref PropKey k, IntPtr v);
    [PreserveSig] int SetValue(ref PropKey k, ref PropVar v);
    [PreserveSig] int Commit();
  }
  [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumeratorCom {}
  [StructLayout(LayoutKind.Sequential)] struct PropKey { public Guid fmtid; public int pid; }
  [StructLayout(LayoutKind.Explicit, Size = 24)] struct PropVar { [FieldOffset(0)] public ushort vt; [FieldOffset(8)] public IntPtr p; }

  public static class GatitoAudio {
    // flow: 0 = speaker (render), 1 = mic (capture)
    public static string DefaultId(int flow) {
      IMMDeviceEnumerator e = (IMMDeviceEnumerator)new MMDeviceEnumeratorCom();
      IMMDevice d;
      if (e.GetDefaultAudioEndpoint(flow, 0, out d) != 0) return "";
      string id;
      d.GetId(out id);
      return id;
    }
    public static int SetDefault(string id) {
      IPolicyConfig p = (IPolicyConfig)new PolicyConfigClient();
      int r = 0;
      for (int i = 0; i < 3; i++) { int x = p.SetDefaultEndpoint(id, i); if (x != 0) r = x; }
      return r;
    }
    public static int Rename(string id, string name) {
      IMMDeviceEnumerator e = (IMMDeviceEnumerator)new MMDeviceEnumeratorCom();
      IMMDevice d;
      int h = e.GetDevice(id, out d);
      if (h != 0) return h;
      IPropertyStore s;
      h = d.OpenPropertyStore(1, out s);
      if (h != 0) return h;
      PropKey k = new PropKey();
      k.fmtid = new Guid("a45c254e-df1c-4efd-8020-67d146a850e0");
      k.pid = 2;
      PropVar v = new PropVar();
      v.vt = 31;
      v.p = Marshal.StringToCoTaskMemUni(name);
      h = s.SetValue(ref k, ref v);
      if (h == 0) h = s.Commit();
      Marshal.FreeCoTaskMem(v.p);
      return h;
    }
  }
}
'@
$MM = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio'
function EpName($flow, $id) {
  try {
    $g = ($id -split '\.')[-1]
    $p = Get-ItemProperty ($MM + '\' + $flow + '\' + $g + '\Properties')
    $d = $p.'{a45c254e-df1c-4efd-8020-67d146a850e0},2'
    $a = $p.'{b3f8fa53-0004-438e-9003-51a46e139bfc},6'
    if ($a) { return ($d + ' (' + $a + ')') } else { return [string]$d }
  } catch { return '' }
}
function FindEp($flow, $rx) {
  if ($flow -eq 'Render') { $pre = '{0.0.0.00000000}' } else { $pre = '{0.0.1.00000000}' }
  foreach ($k in (Get-ChildItem ($MM + '\' + $flow) -ErrorAction SilentlyContinue)) {
    try {
      $s = (Get-ItemProperty $k.PSPath).DeviceState
      if ($s -ne 1) { continue }
      $n = EpName $flow $k.PSChildName
      if ($n -match $rx) { return ($pre + '.' + $k.PSChildName) }
    } catch {}
  }
  return ''
}
