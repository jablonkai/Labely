#!/bin/sh
# Rebuilds the desktop icons from labely.svg. Needs rsvg-convert, ImageMagick and, for the
# .icns, macOS iconutil. The outputs are committed, so this only runs when the mark changes.
set -eu
cd "$(dirname "$0")"
for s in 32 128 256 512; do rsvg-convert -w $s -h $s labely.svg -o ${s}x${s}.png; done
magick 256x256.png -define icon:auto-resize=256,128,64,48,32,16 labely.ico
# macOS: the mark sits on the 824 px grid of a 1024 px canvas.
set=$(mktemp -d)/labely.iconset
mkdir -p "$set"
rsvg-convert -w 824 -h 824 labely.svg -o "$set/mark.png"
magick -size 1024x1024 xc:none "$set/mark.png" -gravity center -composite "$set/base.png"
for s in 16 32 128 256 512; do
  magick "$set/base.png" -resize ${s}x${s} "$set/icon_${s}x${s}.png"
  d=$((s * 2))
  magick "$set/base.png" -resize ${d}x${d} "$set/icon_${s}x${s}@2x.png"
done
rm "$set/mark.png" "$set/base.png"
iconutil -c icns "$set" -o labely.icns
