#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build the GuestKit CLI/TUI demo video from 4 separate VHS recordings
# (rec-inspect.tape, rec-doctor.tape, rec-migrate.tape, rec-tui.tape) +
# title/caption cards (render-cards.mjs).
#
# Usage:
#   node render-cards.mjs
#   vhs rec-inspect.tape   # -> raw/inspect-raw.mp4
#   vhs rec-doctor.tape    # -> raw/doctor-raw.mp4
#   vhs rec-migrate.tape   # -> raw/migrate-raw.mp4
#   vhs rec-tui.tape       # -> raw/tui-raw.mp4
#   ./build.sh
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

make_title  "00-title.mp4"      3.5 "title-main"
make_title  "01-t-inspect.mp4"  1.8 "title-inspect"
extract_clip "raw/inspect-raw.mp4" 3.8 12.5 "02-inspect.mp4" "cap-inspect"
make_title  "03-t-doctor.mp4"   1.8 "title-doctor"
extract_clip "raw/doctor-raw.mp4"  3.8 18.0 "04-doctor.mp4"  "cap-doctor"
make_title  "05-t-migrate.mp4"  1.8 "title-migrate"
extract_clip "raw/migrate-raw.mp4" 3.8 10.5 "06-migrate.mp4" "cap-migrate"
make_title  "07-t-tui.mp4"      1.8 "title-tui"
extract_clip "raw/tui-raw.mp4"     3.8 7.0  "08-tui.mp4"     "cap-tui"
make_title  "09-outro.mp4"      3.5 "outro"

LIST=seg/list.txt
: > "${LIST}"
for f in 00-title 01-t-inspect 02-inspect 03-t-doctor 04-doctor \
         05-t-migrate 06-migrate 07-t-tui 08-tui 09-outro; do
  echo "file '${f}.mp4'" >> "${LIST}"
done

ffmpeg -y -f concat -safe 0 -i "${LIST}" -c copy out/guestkit-cli-demo.mp4 -loglevel error
echo "== built out/guestkit-cli-demo.mp4 =="
