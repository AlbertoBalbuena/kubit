# Attribution

Code adapted from other projects is recorded here (CLAUDE.md, rule 9): source, commit and license.
Only Apache-2.0, MIT, BSD, ISC and Zlib code may be adapted. Nothing is adapted yet.

Columns: **source** (repository URL), **commit** (full SHA adapted from), **license**, **files**
(Kubit files containing adapted code, and what was adapted).

## SolidCraft
| source | commit | license | files |
|--------|--------|---------|-------|

## truck
| source | commit | license | files |
|--------|--------|---------|-------|

## monstertruck
| source | commit | license | files |
|--------|--------|---------|-------|

## solidcore
| source | commit | license | files |
|--------|--------|---------|-------|

## arris
| source | commit | license | files |
|--------|--------|---------|-------|

## Test data

### NIST PMI STEP files (kernel spike corpus)
Used by the kernel spike in `spikes/kernel`; the files are not committed
(`spikes/kernel/fixtures/step/fetch-nist.sh` restores them).

- Source: https://www.nist.gov/el/systems-integration-division-73400/mbe-pmi-validation-and-conformance-testing/download-free-cad
- Archive: `NIST-PMI-STEP-Files.zip`, SHA-256 `8fa78429e6d8d9b0d7681d223b6aa9ec98c3772185c55b1a0e3679b21c181911`
- Terms (NIST): the test cases, CAD models and STEP files can be used without any restrictions.
  NIST asks for acknowledgement; its logo must not be used in promotional material.
- Credit: National Institute of Standards and Technology (NIST), Engineering Laboratory.
- Selected: ctc_01 to ctc_05, ftc_06, ftc_07, ftc_08, ftc_10, ftc_11.
