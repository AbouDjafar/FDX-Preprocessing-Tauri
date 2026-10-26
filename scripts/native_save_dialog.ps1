param(
    [Parameter(Mandatory)][int]$ApplicationId,
    [Parameter(Mandatory)][string]$Destination
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Native UI verification is restricted to GitHub Actions.' }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class FdxNativePicker {
    [DllImport("user32.dll", EntryPoint="SendMessageW", CharSet=CharSet.Unicode)]
    public static extern IntPtr SetText(IntPtr window, uint message, UIntPtr parameter, string text);
    [DllImport("user32.dll", EntryPoint="SendMessageW")]
    public static extern IntPtr Click(IntPtr window, uint message, UIntPtr parameter, IntPtr data);
    private delegate bool EnumChild(IntPtr window, IntPtr data);
    [DllImport("user32.dll")]
    private static extern bool EnumChildWindows(IntPtr parent, EnumChild callback, IntPtr data);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    private static extern int GetClassNameW(IntPtr window, StringBuilder name, int size);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    private static extern int GetWindowTextW(IntPtr window, StringBuilder text, int size);
    public static string ClassName(IntPtr window) {
        var name = new StringBuilder(256);
        GetClassNameW(window, name, name.Capacity);
        return name.ToString();
    }
    public static string Text(IntPtr window) {
        var text = new StringBuilder(32768);
        GetWindowTextW(window, text, text.Capacity);
        return text.ToString();
    }
    public static IntPtr FindEdit(IntPtr parent) {
        if (ClassName(parent).Equals("Edit", StringComparison.OrdinalIgnoreCase)) return parent;
        var result = IntPtr.Zero;
        EnumChildWindows(parent, (window, data) => {
            if (!ClassName(window).Equals("Edit", StringComparison.OrdinalIgnoreCase)) return true;
            result = window;
            return false;
        }, IntPtr.Zero);
        return result;
    }
}
'@
$automation = [System.Windows.Automation.AutomationElement]
$scope = [System.Windows.Automation.TreeScope]::Descendants
$condition = New-Object System.Windows.Automation.AndCondition @(
    (New-Object System.Windows.Automation.PropertyCondition $automation::ProcessIdProperty,$ApplicationId),
    (New-Object System.Windows.Automation.PropertyCondition $automation::ClassNameProperty,'#32770')
)
$deadline = [DateTime]::UtcNow.AddSeconds(30)
$reported = $false
while ([DateTime]::UtcNow -lt $deadline) {
    $dialog = $automation::RootElement.FindFirst($scope,$condition)
    if ($null -ne $dialog) {
        $fileName = $null
        $hostControl = $dialog.FindFirst($scope,(New-Object System.Windows.Automation.PropertyCondition $automation::AutomationIdProperty,'FileNameControlHost'))
        if ($null -ne $hostControl) {
            $fileName = $hostControl.FindFirst($scope,(New-Object System.Windows.Automation.PropertyCondition $automation::AutomationIdProperty,'1001'))
        }
        $save = $dialog.FindFirst($scope,(New-Object System.Windows.Automation.PropertyCondition $automation::AutomationIdProperty,'1'))
        if (-not $reported) {
            $reported = $true
            foreach ($element in $dialog.FindAll($scope,[System.Windows.Automation.Condition]::TrueCondition)) {
                $item = $element.Current
                Write-Output ($item.ControlType.ProgrammaticName + ' | ' + $item.AutomationId + ' | ' + $item.Name)
            }
        }
        if ($null -ne $fileName -and $null -ne $save) {
            $nameHandle = [IntPtr]::new([long]$fileName.Current.NativeWindowHandle)
            $saveHandle = [IntPtr]::new([long]$save.Current.NativeWindowHandle)
            if ($nameHandle -eq [IntPtr]::Zero -or $saveHandle -eq [IntPtr]::Zero) { throw 'Native file picker control handles are unavailable.' }
            $editHandle = [FdxNativePicker]::FindEdit($nameHandle)
            if ($editHandle -eq [IntPtr]::Zero) { throw ('Filename editor unavailable in ' + [FdxNativePicker]::ClassName($nameHandle)) }
            # WM_SETTEXT and BM_CLICK also work when Windows exposes legacy
            # common-dialog controls as panes without UIA Value/Invoke patterns.
            $set = [FdxNativePicker]::SetText($editHandle,0x000C,[UIntPtr]::Zero,$Destination)
            if ($set -eq [IntPtr]::Zero) { throw 'Windows rejected the output filename.' }
            if ([FdxNativePicker]::Text($editHandle) -ne $Destination) { throw 'Windows did not retain the requested output filename.' }
            [void][FdxNativePicker]::Click($saveHandle,0x00F5,[UIntPtr]::Zero,[IntPtr]::Zero)
            Write-Output 'Native Windows save-file picker completed.'
            exit 0
        }
    }
    Start-Sleep -Milliseconds 250
}
throw 'The native Windows save-file picker did not become accessible within 30 seconds.'
