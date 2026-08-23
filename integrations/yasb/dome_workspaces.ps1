param(
    [Parameter(Mandatory = $true)][string] $Monitor,
    [string] $DomePath = 'dome'
)

function Get-MonitorSlug([string] $name) {
    return ($name.ToLowerInvariant() -replace '[^a-z0-9]+', '-').Trim('-')
}

# Colours, edit to match your theme.
$focused_bg   = '#cba6f7'
$focused_fg   = '#1e1e2e'
$visible_bg   = '#45475a'
$visible_fg   = '#cdd6f4'
$occupied_bg  = '#313244'
$occupied_fg  = '#bac2de'
$parked_fg    = '#f9e2af'
$separator_fg = '#585b70'
$pill_style   = 'padding:2px 8px;text-align:center;vertical-align:middle'
$sep_style    = 'padding:2px 3px;vertical-align:middle'

try {
    $raw  = & $DomePath query workspaces 2>$null | Out-String
    $data = $raw | ConvertFrom-Json
} catch {
    Write-Output ""
    exit 0
}

# A query timeout answers {"error":...} rather than the array. Bail so the bar keeps
# its last content instead of blanking. Windows PowerShell unwraps a one-element
# array, so a single workspace arrives as an object, which the wrap below normalises.
if ($null -eq $data -or ($data.PSObject.Properties.Name -contains 'error')) {
    Write-Output ""
    exit 0
}
$data = @($data)

# A parked workspace belongs to a monitor that is not present, so the indicator is
# not scoped to this bar. Only a parked workspace that still holds windows earns it.
$has_parked = @($data | Where-Object {
    $_.state -eq 'Parked' -and $_.window_count -gt 0
}).Count -gt 0

# The state test is not redundant with the slug test. A parked row's monitor is a
# frozen origin name, and a rerank can later hand that string to a live monitor.
$data = $data | Where-Object {
    $_.state -eq 'Attached' -and (Get-MonitorSlug $_.monitor) -eq $Monitor
}

$data = $data | Sort-Object { [int]$_.name }

$cells = @()
foreach ($ws in $data) {
    if ($ws.is_focused) {
        $cells += "<td style='background:$focused_bg;color:$focused_fg;$pill_style'>$($ws.name)</td>"
    } elseif ($ws.is_visible) {
        $cells += "<td style='background:$visible_bg;color:$visible_fg;$pill_style'>$($ws.name)</td>"
    } elseif ($ws.window_count -gt 0) {
        $cells += "<td style='background:$occupied_bg;color:$occupied_fg;$pill_style'>$($ws.name)</td>"
    } else {
        continue
    }
    $cells += "<td style='color:$separator_fg;$sep_style'>|</td>"
}
# Drop the trailing separator.
if ($cells.Count -gt 0) { $cells = $cells[0..($cells.Count - 2)] }

if ($has_parked) {
    if ($cells.Count -gt 0) {
        $cells += "<td style='color:$separator_fg;$sep_style'>|</td>"
    }
    $cells += "<td style='color:$parked_fg;$pill_style'>parked</td>"
}

if ($cells.Count -eq 0) {
    Write-Output ""
} else {
    Write-Output "<table cellspacing=0 cellpadding=0><tr>$($cells -join '')</tr></table>"
}
