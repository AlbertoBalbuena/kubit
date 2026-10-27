#!/bin/sh
# Restores the NIST STEP corpus used by spike case 12.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
url="https://www.nist.gov/system/files/documents/noindex/2024/06/19/NIST-PMI-STEP-Files.zip"
sha="8fa78429e6d8d9b0d7681d223b6aa9ec98c3772185c55b1a0e3679b21c181911"
curl -sSL -A "Mozilla/5.0" -o "$tmp/nist.zip" "$url"
echo "$sha  $tmp/nist.zip" | shasum -a 256 -c -
unzip -q "$tmp/nist.zip" -d "$tmp"
mkdir -p "$here/nist"
for f in ctc_01_asme1_ap242-e1 ctc_02_asme1_ap242-e2 ctc_03_asme1_ap242-e2 ctc_04_asme1_ap242-e1 \
         ctc_05_asme1_ap242-e1 ftc_06_asme1_ap242-e2 ftc_07_asme1_ap242-e2 ftc_08_asme1_ap242-e1-tg \
         ftc_10_asme1_ap242-e2 ftc_11_asme1_ap242-e2; do
  cp "$tmp/NIST-PMI-STEP-Files/nist_$f.stp" "$here/nist/"
done
rm -rf "$tmp"
