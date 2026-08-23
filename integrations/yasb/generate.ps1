<#
.SYNOPSIS
Emit the YASB bars and widgets that scope Dome's workspaces to each monitor.

.DESCRIPTION
Reads `dome query monitors` and prints two marked YAML blocks to stdout, one bars:
block and one widgets: block, with one entry per connected monitor. Nothing is
written to disk. Paste the two blocks into config.yaml, replacing the blocks from
the previous run. Re-run when the monitor set changes.

.PARAMETER ScriptPath
Path the generated run_cmd points at for the formatter. Defaults to the location
the docs recommend. run_cmd is split on spaces, so the path cannot contain one.

.PARAMETER DomePath
The dome binary baked into every run_cmd and used for the query here. Pass an
absolute path to remove any dependence on PATH. Defaults to a PATH lookup.
#>
param(
    [string] $ScriptPath = "$env:USERPROFILE\.config\yasb\dome_workspaces.ps1",
    [string] $DomePath = 'dome'
)

function Get-MonitorSlug([string] $name) {
    return ($name.ToLowerInvariant() -replace '[^a-z0-9]+', '-').Trim('-')
}

# A single-quoted YAML scalar. Its one escape is '' for a literal apostrophe, and no
# character inside it is an indicator, so a Windows path's backslashes pass through
# as written. Double every apostrophe unconditionally rather than reasoning about
# which values can hold one.
function ConvertTo-YamlScalar([string] $value) {
    return "'" + ($value -replace "'", "''") + "'"
}

function Stop-Generator([string] $message) {
    [Console]::Error.WriteLine("generate.ps1: $message")
    exit 1
}

# run_cmd is split on spaces with no quoting, so a baked path with a space arrives
# truncated at the widget. Refuse rather than emit a config that fails silently.
if ($ScriptPath -match ' ') {
    Stop-Generator "-ScriptPath '$ScriptPath' contains a space. Move the formatter to a path without one."
}
if ($DomePath -match ' ') {
    Stop-Generator "-DomePath '$DomePath' contains a space. Place the dome binary under a path without one."
}

# Fail with a message and print nothing on any query problem, so a partial fragment
# never reaches the clipboard.
$raw = & $DomePath query monitors 2>$null | Out-String
if ($LASTEXITCODE -ne 0) {
    Stop-Generator "'$DomePath query monitors' exited $LASTEXITCODE"
}
try {
    $parsed = $raw | ConvertFrom-Json
} catch {
    Stop-Generator "'$DomePath query monitors' did not return JSON"
}
# A query timeout answers {"error":...} rather than the array. Windows PowerShell
# unwraps a one-element array, so a count cannot tell one monitor from an error
# object, but the error object carries an `error` property and a monitor never does.
if ($null -eq $parsed) {
    Stop-Generator "'$DomePath query monitors' returned nothing"
}
if ($parsed.PSObject.Properties.Name -contains 'error') {
    Stop-Generator "'$DomePath query monitors' failed: $($parsed.error)"
}
$monitors = @($parsed)

# Refuse a duplicate slug rather than emit two bars with the same key, which YAML
# collapses to the last, silently dropping a monitor's bar.
$seen = @{}
foreach ($m in $monitors) {
    $slug = Get-MonitorSlug $m.unique_name
    if ($slug -eq '') {
        Stop-Generator "monitor '$($m.unique_name)' slugs to an empty token"
    }
    if ($seen.ContainsKey($slug)) {
        Stop-Generator "'$($m.unique_name)' and '$($seen[$slug])' both slug to '$slug'. Rename one monitor."
    }
    $seen[$slug] = $m.unique_name
}

# device_name repeats across identical panels. $null + 1 is 1 in PowerShell, so a
# first sighting seeds the count without a ContainsKey guard.
$device_counts = @{}
foreach ($m in $monitors) {
    $device_counts[$m.device_name] += 1
}

$lines = @()
$lines += '# >>> dome generated bars, replace this whole block when you re-run generate.ps1 >>>'
$lines += '# Merge these entries INTO the bars: mapping your config.yaml already has. Do not'
$lines += '# paste a second bars: key, YAML keeps only the last one and your other bars vanish'
$lines += '# with no error. A bar of your own that uses screens: [*] stops drawing once these'
$lines += '# bars claim every screen.'
$lines += 'bars:'
foreach ($m in $monitors) {
    $slug = Get-MonitorSlug $m.unique_name
    $widget = 'dome_workspaces_' + ($slug -replace '-', '_')
    $lines += "  dome-bar-${slug}:"
    $lines += '    enabled: true'
    $lines += "    screens: [$(ConvertTo-YamlScalar $m.device_name)]"
    if ($device_counts[$m.device_name] -gt 1) {
        $lines += "    # '$($m.device_name)' repeats. Append Qt's (N) suffix to this screens: value"
        $lines += '    # by hand. YASB logs the real names in a "screen not found" warning.'
    }
    $lines += '    widgets:'
    $lines += "      left: [$(ConvertTo-YamlScalar $widget)]"
}
$lines += '# <<< dome generated bars <<<'
$lines += ''
$lines += '# >>> dome generated widgets, replace this whole block when you re-run generate.ps1 >>>'
$lines += 'widgets:'
foreach ($m in $monitors) {
    $slug = Get-MonitorSlug $m.unique_name
    $widget = 'dome_workspaces_' + ($slug -replace '-', '_')
    $run_cmd = "powershell -ExecutionPolicy Bypass -File $ScriptPath -Monitor $slug -DomePath $DomePath"
    $lines += "  ${widget}:"
    $lines += "    type: 'yasb.custom.CustomWidget'"
    $lines += '    options:'
    $lines += "      label: '{data}'"
    $lines += "      label_alt: '{data}'"
    $lines += "      class_name: 'dome-workspaces-widget'"
    $lines += '      exec_options:'
    $lines += "        run_cmd: $(ConvertTo-YamlScalar $run_cmd)"
    $lines += '        run_interval: 1000'
    $lines += "        return_format: 'string'"
}
$lines += '# <<< dome generated widgets <<<'

Write-Output ($lines -join "`n")
