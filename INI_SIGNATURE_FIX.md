# INI Signature Extraction Fix

## Issue
The user reported a discrepancy where an imported INI file was identified as "rusEFI master" in the repository list (EXACT match), but as "uaefiBIGFUEL" when loaded by the application (causing a signature mismatch error).

## Root Cause
The `ini_repository` parser was naive and scanned the entire file for `signature = ...` lines without respecting INI sections. The `core parser` (used for loading) correctly respects the `[MegaTune]` section.
The user's INI file likely contains the correct signature (`uaefiBIGFUEL`) inside the `[MegaTune]` section, but a stale or comment-like signature (`rusEFI master`) appearing later in the file (e.g. in a changelog or incorrectly commented section).
This caused the Repository to index it as "rusEFI master" (last match wins), while the App loaded it as "uaefiBIGFUEL".

## Fix
Updated `crates/libretune-core/src/project/repository.rs`:
- Implemented section-aware parsing in `extract_ini_info`.
- parser now tracks `[MegaTune]` section entry/exit.
- Only accepts `signature` and `nEmu` fields strictly within the `[MegaTune]` section.
- Also improved value extraction to handle values containing `=` signs (using `splitn(2, '=')` instead of `split('=')`).

## Impact
- Consistency: Repository index and App loader will now always agree on the signature.
- UX: Malformed INI files will be correctly identified in the import list, preventing user confusion.
