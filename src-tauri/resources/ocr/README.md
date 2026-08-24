# OCR runtime release contract

A Windows release must vendor and checksum:
- Tesseract executable and required DLLs
- Poppler `pdftotext` and `pdftoppm` plus required DLLs
- `heb.traineddata`
- `ara.traineddata`
- `eng.traineddata`

The extraction layer never runs against a mutable original source. It first creates a
private snapshot, hashes the exact snapshot bytes, compares them with the indexed
DocumentVersion SHA256, and verifies the snapshot again immediately before persistence.

This reconstructed source intentionally does not ship third-party binaries.
The Windows vendor script must populate this directory before packaging.
