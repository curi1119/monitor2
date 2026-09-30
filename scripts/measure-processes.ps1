param(
    [Parameter(Mandatory)][int]$LegacyProcessId,
    [Parameter(Mandatory)][int]$PrototypeProcessId,
    [ValidateRange(10,3600)][int]$DurationSeconds = 120,
    [ValidateRange(1,30)][int]$IntervalSeconds = 2,
    [ValidateRange(0,60)][int]$WarmupSeconds = 10,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../target/benchmarks')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
if ($WarmupSeconds -gt 0) { Start-Sleep -Seconds $WarmupSeconds }
$logicalProcessors = [Environment]::ProcessorCount
$startedAt = [DateTimeOffset]::Now
$clock = [Diagnostics.Stopwatch]::StartNew()
$rows = [Collections.Generic.List[object]]::new()
do {
    $elapsed = $clock.Elapsed.TotalSeconds
    foreach ($entry in @(@{ Label='legacy'; Id=$LegacyProcessId }, @{ Label='prototype'; Id=$PrototypeProcessId })) {
        $process = Get-Process -Id $entry.Id
        $process.Refresh()
        $rows.Add([pscustomobject]@{
            Label=$entry.Label; ProcessId=$entry.Id; ElapsedSeconds=$elapsed
            CPUSeconds=$process.TotalProcessorTime.TotalSeconds
            WorkingSetMiB=$process.WorkingSet64 / 1MB
            PrivateMiB=$process.PrivateMemorySize64 / 1MB
            Handles=$process.HandleCount
        })
    }
    if ($clock.Elapsed.TotalSeconds -ge $DurationSeconds) { break }
    Start-Sleep -Seconds $IntervalSeconds
} while ($true)
$summary = foreach ($label in @('legacy','prototype')) {
    $samples = @($rows | Where-Object Label -eq $label)
    $first=$samples[0]; $last=$samples[-1]
    $duration=$last.ElapsedSeconds-$first.ElapsedSeconds
    $cpuDelta=$last.CPUSeconds-$first.CPUSeconds
    $working=$samples | Measure-Object WorkingSetMiB -Minimum -Maximum -Average
    $private=$samples | Measure-Object PrivateMiB -Minimum -Maximum -Average
    [pscustomobject]@{
        Label=$label; ProcessId=$first.ProcessId; Samples=$samples.Count
        DurationSeconds=[Math]::Round($duration,3)
        CPUSeconds=[Math]::Round($cpuDelta,6)
        CPUPercentOneCore=[Math]::Round($cpuDelta/$duration*100,4)
        CPUPercentWholeMachine=[Math]::Round($cpuDelta/$duration*100/$logicalProcessors,4)
        WorkingSetMiBMean=[Math]::Round($working.Average,3)
        WorkingSetMiBMin=[Math]::Round($working.Minimum,3)
        WorkingSetMiBMax=[Math]::Round($working.Maximum,3)
        PrivateMiBMean=[Math]::Round($private.Average,3)
        PrivateMiBMin=[Math]::Round($private.Minimum,3)
        PrivateMiBMax=[Math]::Round($private.Maximum,3)
        PrivateMiBStart=[Math]::Round($first.PrivateMiB,3)
        PrivateMiBEnd=[Math]::Round($last.PrivateMiB,3)
        HandlesStart=$first.Handles; HandlesEnd=$last.Handles
    }
}
$result = [pscustomobject]@{StartedAt=$startedAt.ToString('o'); LogicalProcessors=$logicalProcessors; Summary=@($summary)}
$rows | Export-Csv -LiteralPath (Join-Path $OutputDirectory 'samples.csv') -NoTypeInformation -Encoding utf8
$json=$result | ConvertTo-Json -Depth 5
[IO.File]::WriteAllText((Join-Path $OutputDirectory 'summary.json'),$json,[Text.UTF8Encoding]::new($false))
Write-Output $json
