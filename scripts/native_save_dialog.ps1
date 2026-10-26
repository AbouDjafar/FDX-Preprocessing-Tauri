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
public static class FdxNativePicker {
    [DllImport("user32.dll", EntryPoint="SendMessageW", CharSet=CharSet.Unicode)]
    public static extern IntPtr SetText(IntPtr window, uint message, UIntPtr parameter, string text);
    [DllImport("user32.dll", EntryPoint="SendMessageW")]
    public static extern IntPtr Click(IntPtr window, uint message, UIntPtr parameter, IntPtr data);
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
            # WM_SETTEXT and BM_CLICK also work when Windows exposes legacy
            # common-dialog controls as panes without UIA Value/Invoke patterns.
            $set = [FdxNativePicker]::SetText($nameHandle,0x000C,[UIntPtr]::Zero,$Destination)
            if ($set -eq [IntPtr]::Zero) { throw 'Windows rejected the output filename.' }
            [void][FdxNativePicker]::Click($saveHandle,0x00F5,[UIntPtr]::Zero,[IntPtr]::Zero)
            Write-Output 'Native Windows save-file picker completed.'
            exit 0
        }
    }
    Start-Sleep -Milliseconds 250
}
throw 'The native Windows save-file picker did not become accessible within 30 seconds.'
