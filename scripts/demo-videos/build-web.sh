#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build the GuestKit web-dashboard tour + tutorial videos from Playwright
# recordings (rec-web-tour.mjs, rec-web-tutorial.mjs) + title/caption cards
# (render-cards-web.mjs).
#
# Usage:
#   node render-cards-web.mjs
#   node rec-web-tour.mjs      # -> raw/web-tour/*.webm
#   node rec-web-tutorial.mjs  # -> raw/web-tutorial/*.webm
#   cp raw/web-tour/*.webm web-tour-raw.webm
#   cp raw/web-tutorial/*.webm web-tutorial-raw.webm
#   ./build-web.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

W=1920
H=1080
FPS=30
CRF=16
PRESET=slow
FADE=0.3

mkdir -p seg out

make_title() {
  local out="$1" dur="$2" png="$3"
  local fout
  fout=$(python3 -c "print(${dur} - ${FADE})")
  ffmpeg -y -loop 1 -t "${dur}" -i "png/${png}.png" \
    -vf "fade=t=in:st=0:d=${FADE},fade=t=out:st=${fout}:d=${FADE}" \
    -r "${FPS}" -c:v libx264 -preset "${PRESET}" -crf "${CRF}" -pix_fmt yuv420p -movflags +faststart "seg/${out}" -loglevel error
  echo "  title: ${out} (${dur}s)"
}

extract_clip() {
  local src="$1" start="$2" dur="$3" out="$4" cap="$5"
  local tmp="seg/_raw_${out}"
  local fout
  fout=$(python3 -c "print(${dur} - ${FADE})")
  ffmpeg -y -ss "${start}" -t "${dur}" -i "${src}" \
    -vf "scale=${W}:${H}:force_original_aspect_ratio=decrease:flags=lanczos,pad=${W}:${H}:(ow-iw)/2:(oh-ih)/2,fps=${FPS},format=yuv420p,fade=t=in:st=0:d=${FADE},fade=t=out:st=${fout}:d=${FADE}" \
    -an -c:v libx264 -preset "${PRESET}" -crf "${CRF}" -pix_fmt yuv420p -movflags +faststart "${tmp}" -loglevel error
  ffmpeg -y -i "${tmp}" -loop 1 -i "png/${cap}.png" \
    -filter_complex "[0:v][1:v] overlay=(W-w)/2:H-h-60:shortest=1" \
    -c:v libx264 -preset "${PRESET}" -crf "${CRF}" -pix_fmt yuv420p -movflags +faststart "seg/${out}" -loglevel error
  rm -f "${tmp}"
  echo "  clip: ${out} src=${src} [${start}, +${dur}]"
}

echo "== Tour =="
TSRC=web-tour-raw.webm
make_title  "wt00-title.mp4"    3.5 "tour-title-main"
extract_clip "${TSRC}" 2.0  2.3 "wt01-landing.mp4"  "cap-landing"
make_title  "wt02-t-cluster.mp4" 1.6 "tour-title-cluster"
extract_clip "${TSRC}" 7.0  2.3 "wt03-cluster.mp4"  "cap-cluster"
extract_clip "${TSRC}" 10.5 1.9 "wt04-selected.mp4" "cap-selected"
make_title  "wt05-t-actions.mp4" 1.6 "tour-title-actions"
extract_clip "${TSRC}" 14.0 2.2 "wt06-inspect.mp4"  "cap-inspect"
extract_clip "${TSRC}" 17.5 2.2 "wt07-doctor.mp4"   "cap-doctor"
extract_clip "${TSRC}" 21.0 1.5 "wt08-upload.mp4"   "cap-upload"
make_title  "wt09-outro.mp4"    3.0 "tour-outro"

: > seg/tour-list.txt
for f in wt00-title wt01-landing wt02-t-cluster wt03-cluster wt04-selected \
         wt05-t-actions wt06-inspect wt07-doctor wt08-upload wt09-outro; do
  echo "file '${f}.mp4'" >> seg/tour-list.txt
done
ffmpeg -y -f concat -safe 0 -i seg/tour-list.txt -c copy out/guestkit-web-tour.mp4 -loglevel error
echo "== built out/guestkit-web-tour.mp4 =="

echo "== Tutorial =="
USRC=web-tutorial-raw.webm
make_title  "wu00-title.mp4"      3.5 "tut-title-main"
make_title  "wu01-t-intake.mp4"   1.8 "tut-title-intake"
extract_clip "${USRC}" 3.0  3.0 "wu02-intake.mp4"    "cap-landing"
make_title  "wu03-t-sources.mp4"  1.8 "tut-title-sources"
extract_clip "${USRC}" 19.0 2.2 "wu04-upload-tab.mp4" "cap-upload"
extract_clip "${USRC}" 39.0 2.5 "wu05-source-tabs.mp4" "cap-upload"
make_title  "wu06-t-cluster.mp4"  1.8 "tut-title-cluster"
extract_clip "${USRC}" 59.5 1.8 "wu07-cluster.mp4"   "cap-cluster"
make_title  "wu08-t-select.mp4"   1.6 "tut-title-select"
extract_clip "${USRC}" 63.0 1.8 "wu09-select.mp4"    "cap-selected"
make_title  "wu10-t-inspect.mp4"  1.8 "tut-title-inspect"
extract_clip "${USRC}" 67.5 1.8 "wu11-inspect.mp4"   "cap-inspect"
make_title  "wu12-t-doctor.mp4"   1.8 "tut-title-doctor"
extract_clip "${USRC}" 72.0 1.8 "wu13-doctor.mp4"    "cap-doctor"
make_title  "wu14-outro.mp4"      3.5 "tut-outro"

: > seg/tutorial-list.txt
for f in wu00-title wu01-t-intake wu02-intake wu03-t-sources wu04-upload-tab wu05-source-tabs \
         wu06-t-cluster wu07-cluster wu08-t-select wu09-select \
         wu10-t-inspect wu11-inspect wu12-t-doctor wu13-doctor wu14-outro; do
  echo "file '${f}.mp4'" >> seg/tutorial-list.txt
done
ffmpeg -y -f concat -safe 0 -i seg/tutorial-list.txt -c copy out/guestkit-web-tutorial.mp4 -loglevel error
echo "== built out/guestkit-web-tutorial.mp4 =="
