$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$assets = Join-Path $projectRoot 'installer\assets'
New-Item -ItemType Directory -Path $assets -Force | Out-Null
Add-Type -AssemblyName System.Drawing

function Draw-Shield([System.Drawing.Graphics]$Graphics, [single]$X, [single]$Y, [single]$Size) {
    $shape = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $shape.AddPolygon([System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new($X + $Size * 0.5, $Y),
        [System.Drawing.PointF]::new($X + $Size * 0.9, $Y + $Size * 0.16),
        [System.Drawing.PointF]::new($X + $Size * 0.84, $Y + $Size * 0.61),
        [System.Drawing.PointF]::new($X + $Size * 0.68, $Y + $Size * 0.84),
        [System.Drawing.PointF]::new($X + $Size * 0.5, $Y + $Size),
        [System.Drawing.PointF]::new($X + $Size * 0.32, $Y + $Size * 0.84),
        [System.Drawing.PointF]::new($X + $Size * 0.16, $Y + $Size * 0.61),
        [System.Drawing.PointF]::new($X + $Size * 0.1, $Y + $Size * 0.16)
    ))
    $brush = [System.Drawing.Drawing2D.LinearGradientBrush]::new([System.Drawing.PointF]::new($X,$Y), [System.Drawing.PointF]::new(($X+$Size),($Y+$Size)), [System.Drawing.ColorTranslator]::FromHtml('#467BFF'), [System.Drawing.ColorTranslator]::FromHtml('#2CD9B6'))
    $Graphics.FillPath($brush, $shape)
    $pen = [System.Drawing.Pen]::new([System.Drawing.Color]::White, ($Size * 0.065))
    $pen.StartCap = 'Round'; $pen.EndCap = 'Round'; $pen.LineJoin = 'Round'
    $Graphics.DrawLines($pen, [System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new($X+$Size*0.31,$Y+$Size*0.48),
        [System.Drawing.PointF]::new($X+$Size*0.46,$Y+$Size*0.63),
        [System.Drawing.PointF]::new($X+$Size*0.71,$Y+$Size*0.35)
    ))
    $pen.Dispose(); $brush.Dispose(); $shape.Dispose()
}

$banner = [System.Drawing.Bitmap]::new(656,1256)
$g = [System.Drawing.Graphics]::FromImage($banner)
$g.SmoothingMode = 'AntiAlias'; $g.TextRenderingHint = 'AntiAliasGridFit'
$gradient = [System.Drawing.Drawing2D.LinearGradientBrush]::new([System.Drawing.Point]::new(0,0),[System.Drawing.Point]::new(656,1256),[System.Drawing.ColorTranslator]::FromHtml('#0C1730'),[System.Drawing.ColorTranslator]::FromHtml('#17386B'))
$g.FillRectangle($gradient,0,0,656,1256)
$line = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(22,120,180,255),2)
foreach ($radius in @(220,340,460,580)) { $g.DrawEllipse($line, [single](328-$radius), [single](380-$radius), [single]($radius*2), [single]($radius*2)) }
Draw-Shield $g 188 215 280
$white = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White)
$soft = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#B6C8E9'))
$accent = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#5FE4CD'))
$brand = [System.Drawing.Font]::new('Segoe UI',52,[System.Drawing.FontStyle]::Bold,[System.Drawing.GraphicsUnit]::Pixel)
$heading = [System.Drawing.Font]::new('Segoe UI',42,[System.Drawing.FontStyle]::Regular,[System.Drawing.GraphicsUnit]::Pixel)
$small = [System.Drawing.Font]::new('Segoe UI',24,[System.Drawing.FontStyle]::Regular,[System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString('STRAJA',$brand,$white,58,590)
$g.DrawString('BACKUP',$small,$accent,62,660)
$g.FillRectangle($accent,62,742,55,4)
$g.DrawString("Your data.`nSafe with you.",$heading,$white,58,790)
$g.DrawString('Created by',$small,$soft,62,950)
$creatorLogo = [System.Drawing.Image]::FromFile((Join-Path $assets 'weblike-pro.png'))
$g.InterpolationMode = 'HighQualityBicubic'
$g.DrawImage($creatorLogo,62,995,510,92)
$creatorLogo.Dispose()
$g.DrawString('strajabackup.com',$small,$soft,62,1100)
$g.DrawString('FOR WINDOWS',$small,$soft,62,1152)
$banner.Save((Join-Path $assets 'wizard.png'),[System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $banner.Dispose(); $gradient.Dispose(); $line.Dispose()
$brand.Dispose(); $heading.Dispose(); $small.Dispose(); $white.Dispose(); $soft.Dispose(); $accent.Dispose()

# PNG-backed, multi-resolution ICO: crisp Explorer, setup and Add/Remove icons.
$frames = @()
foreach ($size in @(16,24,32,48,64,128,256)) {
    $bitmap = [System.Drawing.Bitmap]::new($size,$size)
    $g = [System.Drawing.Graphics]::FromImage($bitmap); $g.SmoothingMode = 'AntiAlias'
    $g.Clear([System.Drawing.Color]::Transparent)
    Draw-Shield $g ([single]($size*.05)) ([single]($size*.04)) ([single]($size*.9))
    $memory = [System.IO.MemoryStream]::new()
    $bitmap.Save($memory,[System.Drawing.Imaging.ImageFormat]::Png)
    $frames += @{ Size=$size; Bytes=$memory.ToArray() }
    if ($size -eq 128) { $bitmap.Save((Join-Path $assets 'wizard-small.png'),[System.Drawing.Imaging.ImageFormat]::Png) }
    $memory.Dispose(); $g.Dispose(); $bitmap.Dispose()
}
$stream = [System.IO.File]::Create((Join-Path $assets 'straja.ico'))
$writer = [System.IO.BinaryWriter]::new($stream)
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$frames.Count)
$offset = 6 + 16 * $frames.Count
foreach ($frame in $frames) {
    $dimension = if ($frame.Size -eq 256) { 0 } else { $frame.Size }
    $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]32)
    $writer.Write([uint32]$frame.Bytes.Length); $writer.Write([uint32]$offset)
    $offset += $frame.Bytes.Length
}
foreach ($frame in $frames) { $writer.Write([byte[]]$frame.Bytes) }
$writer.Dispose(); $stream.Dispose()
Write-Host 'Identitatea vizuală a instalatorului a fost generată.'

# Embedded tray states and RGBA window icon, generated from the same shield.
foreach ($state in @('ready','busy','error','offline')) {
    $bitmap = [System.Drawing.Bitmap]::new(128,128)
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.SmoothingMode = 'AntiAlias'
    $g.Clear([System.Drawing.Color]::Transparent)
    Draw-Shield $g 6 5 115
    $colors = @{ ready='#16A36A'; busy='#EAA21B'; error='#DC3545'; offline='#788594' }
    $edge = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White)
    $dot = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml($colors[$state]))
    $g.FillEllipse($edge,76,76,48,48)
    $g.FillEllipse($dot,81,81,38,38)
    $bitmap.Save((Join-Path $assets "tray-$state.png"),[System.Drawing.Imaging.ImageFormat]::Png)
    $dot.Dispose(); $edge.Dispose(); $g.Dispose(); $bitmap.Dispose()
}
$bitmap = [System.Drawing.Bitmap]::new((Join-Path $assets 'wizard-small.png'))
$rgba = [byte[]]::new(128*128*4)
for ($y=0; $y -lt 128; $y++) {
    for ($x=0; $x -lt 128; $x++) {
        $pixel = $bitmap.GetPixel($x,$y); $i=($y*128+$x)*4
        $rgba[$i]=$pixel.R; $rgba[$i+1]=$pixel.G; $rgba[$i+2]=$pixel.B; $rgba[$i+3]=$pixel.A
    }
}
[IO.File]::WriteAllBytes((Join-Path $assets 'straja.rgba'),$rgba)
$bitmap.Dispose()
