param(
    [Parameter(Mandatory)][int]$ApplicationId,
    [Parameter(Mandatory)][string]$Destination
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Native UI verification is restricted to GitHub Actions.' }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$automation = [System.Windows.Automation.AutomationElement]
$scope = [System.Windows.Automation.TreeScope]::Descendants
$condition = New-Object System.Windows.Automation.AndCondition @(
    (New-Object System.Windows.Automation.PropertyCondition $automation::ProcessIdProperty,$ApplicationId),
    (New-Object System.Windows.Automation.PropertyCondition $automation::ClassNameProperty,'#32770')
)
$deadline = [DateTime]::UtcNow.AddSeconds(30)
while ([DateTime]::UtcNow -lt $deadline) {
    $dialog = $automation::RootElement.FindFirst($scope,$condition)
    if ($null -ne $dialog) {
        $fileName = $dialog.FindFirst($scope,(New-Object System.Windows.Automation.PropertyCondition $automation::AutomationIdProperty,'1001'))
        $save = $dialog.FindFirst($scope,(New-Object System.Windows.Automation.PropertyCondition $automation::AutomationIdProperty,'1'))
        if ($null -ne $fileName -and $null -ne $save) {
            $value = $fileName.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
            $value.SetValue($Destination)
            $invoke = $save.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
            $invoke.Invoke()
            Write-Output 'Native Windows save-file picker completed.'
            exit 0
        }
    }
    Start-Sleep -Milliseconds 250
}
throw 'The native Windows save-file picker did not become accessible within 30 seconds.'
