# Prints one monitor's dome workspaces as a YASB label.

param(
    # The monitor's unique_name or gdi_device.
    [string]$Monitor = ''
)

$FocusedStyle  = 'background:#cba6f7;color:#1e1e2e'
$VisibleStyle  = 'background:#45475a;color:#cdd6f4'
$OccupiedStyle = 'background:#313244;color:#bac2de'
$ErrorStyle    = 'color:#f38ba8'
$CellStyle     = 'padding:2px 8px;text-align:center;vertical-align:middle'
$Separator     = "<td style='color:#585b70;padding:2px 3px;vertical-align:middle'>|</td>"

# Without this, Windows PowerShell 5.1 garbles a non-ASCII workspace name.
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

# YASB discards stderr, so errors go to the label. Without the <span>, Qt shows an
# encoded quote as &quot;.
function Show-Error([string]$Message) {
    Write-Output "<span style='$ErrorStyle'>$([System.Net.WebUtility]::HtmlEncode($Message))</span>"
    exit 0
}

try {
    $output = @(dome query workspaces --monitor $Monitor 2>&1)
    $errors = @($output | Where-Object { $_ -is [System.Management.Automation.ErrorRecord] })
    if ($LASTEXITCODE -ne 0) {
        $line = $errors | ForEach-Object { "$_" } | Where-Object { $_.Trim() } | Select-Object -First 1
        if (-not $line) { $line = "dome exited with code $LASTEXITCODE" }
        Show-Error $line
    }
    $json = ($output | Where-Object { $_ -isnot [System.Management.Automation.ErrorRecord] }) -join "`n"
    # A ForEach-Object pipeline would get this array as one object in Windows PowerShell 5.1.
    $cells = foreach ($w in ($json | ConvertFrom-Json)) {
        $style = if ($w.is_focused) { $FocusedStyle } elseif ($w.is_visible) { $VisibleStyle } else { $OccupiedStyle }
        "<td style='$style;$CellStyle'>$([System.Net.WebUtility]::HtmlEncode($w.name))</td>"
    }
} catch {
    Show-Error $_.Exception.Message
}

if ($cells) {
    Write-Output "<table cellspacing=0 cellpadding=0><tr>$($cells -join $Separator)</tr></table>"
}
