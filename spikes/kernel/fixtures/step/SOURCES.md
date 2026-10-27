# STEP corpus sources

## nist/
- Archive: `NIST-PMI-STEP-Files.zip` (13,976,599 bytes)
- URL: https://www.nist.gov/system/files/documents/noindex/2024/06/19/NIST-PMI-STEP-Files.zip
  (linked from https://www.nist.gov/el/systems-integration-division-73400/mbe-pmi-validation-and-conformance-testing/download-free-cad)
- SHA-256: `8fa78429e6d8d9b0d7681d223b6aa9ec98c3772185c55b1a0e3679b21c181911`
- Downloaded: 2026-10-26
- Terms (quoted from the NIST page): "The test cases, CAD models, and STEP files can be used without any restrictions."
  NIST asks for acknowledgement and forbids using its logo in promotional material.
- Files are not committed; run `./fetch-nist.sh` to restore them.

Selected (10): ctc_01..05, ftc_06, ftc_07, ftc_08 (e1-tg), ftc_10, ftc_11.

## local/
Your own STEP files (gitignored). Every `*.stp`/`*.step` here is imported by case 12.
