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
$reported = $false
while ([DateTime]::UtcNow -lt $deadline) {
    $dialog = $automation::RootElement.FindFirst($scope,$condition)
    if ($null -ne $dialog) {
        $nameCondition = New-Object System.Windows.Automation.AndCondition @(
            (New-Object System.Windows.Automation.PropertyCondition $automation::AutomationIdProperty,'1001'),
            (New-Object System.Windows.Automation.PropertyCondition $automation::ControlTypeProperty,([System.Windows.Automation.ControlType]::Edit))
        )
        $saveCondition = New-Object System.Windows.Automation.AndCondition @(
            (New-Object System.Windows.Automation.PropertyCondition $automation::AutomationIdProperty,'1'),
            (New-Object System.Windows.Automation.PropertyCondition $automation::ControlTypeProperty,([System.Windows.Automation.ControlType]::Button))
        )
        $fileName = $dialog.FindFirst($scope,$nameCondition)
        if ($null -eq $fileName) {
            $edits = $dialog.FindAll($scope,(New-Object System.Windows.Automation.PropertyCondition $automation::ControlTypeProperty,([System.Windows.Automation.ControlType]::Edit)))
            foreach ($edit in $edits) {
                if ($edit.Current.Name -match 'file name|nom du fichier') { $fileName = $edit; break }
            }
        }
        $save = $dialog.FindFirst($scope,$saveCondition)
        if (-not $reported) {
            $reported = $true
            foreach ($element in $dialog.FindAll($scope,[System.Windows.Automation.Condition]::TrueCondition)) {
                $item = $element.Current
                Write-Output ($item.ControlType.ProgrammaticName + ' | ' + $item.AutomationId + ' | ' + $item.Name)
            }
        }
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
