# dome YASB workspaces plugin. dome writes this file. YASB runs it every tick
# with the monitor name as its argument. It prints the workspace table for
# that monitor. Edit the colors below. A regenerate overwrites the whole file.

param([string]$Monitor = '')

# Colors. Defaults are the Catppuccin Mocha palette.
$FocusedBg   = '#cba6f7'
$FocusedFg   = '#1e1e2e'
$VisibleBg   = '#45475a'
$VisibleFg   = '#cdd6f4'
$OccupiedBg  = '#313244'
$OccupiedFg  = '#bac2de'
$SeparatorFg = '#585b70'
$ParkedFg    = '#f9e2af'

$CellStyle = 'padding:2px 8px;text-align:center;vertical-align:middle'
$SepStyle  = 'padding:2px 3px;vertical-align:middle'

$Dome = __DOME__

# Lowercase, collapse each run of non-alphanumeric characters to one '-', and
# trim leading and trailing '-'. Matches dome's own slug.
function Get-Slug {
    param([string]$Name)
    if ([string]::IsNullOrEmpty($Name)) { return '' }
    $s = $Name.ToLowerInvariant()
    $s = $s -replace '[^a-z0-9]+', '-'
    return $s.Trim('-')
}

function New-WorkspaceCell {
    param([string]$Bg, [string]$Fg, [string]$Name)
    return "<td style='background:$Bg;color:$Fg;$CellStyle'>$Name</td>"
}

# A failed or empty query leaves the bar blank rather than erroring.
$json = & $Dome query workspaces 2>$null | Out-String
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($json)) {
    Write-Output ''
    exit 0
}
try {
    $workspaces = $json | ConvertFrom-Json
} catch {
    Write-Output ''
    exit 0
}
if ($null -eq $workspaces -or ($workspaces -is [PSCustomObject] -and $workspaces.PSObject.Properties.Name -contains 'error')) {
    Write-Output ''
    exit 0
}
if ($workspaces -isnot [Array]) {
    $workspaces = @($workspaces)
}

# The parked indicator is global. A parked workspace's origin monitor is gone,
# so no single bar can own it.
$hasParked = $false
foreach ($w in $workspaces) {
    if ($w.state -eq 'Parked' -and $w.window_count -gt 0) {
        $hasParked = $true
        break
    }
}

# Cells show live workspaces on this monitor. $Monitor is slugged so a name or
# its slug both match. A parked workspace keeps its origin monitor name, so the
# Attached check drops it from the cells.
$want = Get-Slug $Monitor
$rows = @($workspaces | Where-Object {
    $_.state -eq 'Attached' -and (Get-Slug $_.monitor) -eq $want
})

# Numeric names sort numerically, non-numeric names lexically after them.
$sort = @{
    Property = @(
        @{ Expression = { if ($_.name -match '^[+-]?\d+$') { 0 } else { 1 } } }
        @{ Expression = { if ($_.name -match '^[+-]?\d+$') { [long]$_.name } else { $_.name } } }
    )
}
$rows = @($rows | Sort-Object @sort)

$separator = "<td style='color:$SeparatorFg;$SepStyle'>|</td>"
$cells = [System.Collections.Generic.List[string]]::new()
foreach ($w in $rows) {
    if ($w.is_focused) {
        $cell = New-WorkspaceCell $FocusedBg $FocusedFg $w.name
    } elseif ($w.is_visible) {
        $cell = New-WorkspaceCell $VisibleBg $VisibleFg $w.name
    } elseif ($w.window_count -gt 0) {
        $cell = New-WorkspaceCell $OccupiedBg $OccupiedFg $w.name
    } else {
        continue
    }
    $cells.Add($cell)
    $cells.Add($separator)
}
if ($cells.Count -gt 0) {
    $cells.RemoveAt($cells.Count - 1)
}

if ($hasParked) {
    if ($cells.Count -gt 0) {
        $cells.Add($separator)
    }
    $cells.Add("<td style='color:$ParkedFg;$CellStyle'>parked</td>")
}

if ($cells.Count -eq 0) {
    Write-Output ''
} else {
    Write-Output "<table cellspacing=0 cellpadding=0><tr>$(-join $cells)</tr></table>"
}
