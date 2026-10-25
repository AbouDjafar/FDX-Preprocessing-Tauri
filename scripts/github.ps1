param(
    [Parameter(Mandatory)][string]$Route,
    [ValidateSet('GET','POST','PUT','PATCH','DELETE')][string]$Method = 'GET',
    [object]$Body,
    [string]$OutputFile
)
$ErrorActionPreference = 'Stop'
if (!$Route.StartsWith('/') -or $Route.StartsWith('//')) { throw 'A relative GitHub API route is required.' }
$credential = "protocol=https`nhost=github.com`n`n" | git credential fill
$values = @{}
foreach ($line in $credential) {
    $pair = $line -split '=',2
    if ($pair.Length -eq 2) { $values[$pair[0]] = $pair[1] }
}
if (!$values.password) { throw 'GitHub authentication is unavailable. Sign in with Git Credential Manager.' }
try {
    $arguments = @{
        Uri = "https://api.github.com$Route"
        Method = $Method
        Headers = @{Authorization = "Bearer $($values.password)"; Accept = 'application/vnd.github+json'; 'X-GitHub-Api-Version' = '2022-11-28'}
    }
    if ($null -ne $Body) {
        $arguments.ContentType = 'application/json'
        $arguments.Body = ConvertTo-Json -InputObject $Body -Depth 30 -Compress
    }
    if ($OutputFile) {
        if ($Method -ne 'GET' -or $null -ne $Body) { throw 'File downloads require a read-only request.' }
        Invoke-WebRequest @arguments -OutFile $OutputFile
    } else {
        Invoke-RestMethod @arguments
    }
} finally { Remove-Variable credential,values,arguments -ErrorAction SilentlyContinue }
