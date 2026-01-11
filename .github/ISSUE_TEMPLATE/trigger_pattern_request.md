---
name: Trigger Pattern Support Request
about: Request support for a trigger pattern not currently available
title: '[TRIGGER] '
labels: 'documentation, question, wontfix'
assignees: ''

---

## ⚠️ Important Information

**Trigger patterns are implemented in ECU firmware, not in LibreTune.**

LibreTune is a tuning interface that reads available trigger patterns from INI files. The actual trigger decoding happens in real-time on your ECU's microcontroller.

### What This Means

- ❌ LibreTune **cannot** add trigger pattern support directly
- ✅ LibreTune **will automatically** show any pattern added to your ECU firmware and INI file
- 👉 You should request this feature from your **ECU firmware project** (Speeduino, rusEFI, etc.)

### Where to Request Support

**For Speeduino:**
- Forum: https://speeduino.com/forum/
- GitHub: https://github.com/noisymime/speeduino/issues

**For rusEFI:**
- GitHub: https://github.com/rusefi/rusefi/issues
- Forum: https://rusefi.com/forum/

**For other ECUs:**
Contact your ECU manufacturer directly.

---

## If You Still Want to Report This

If you're reporting this to track that a pattern is missing, please provide:

### Trigger Pattern Details
- **Engine/Vehicle**: (e.g., Nissan Micra K12 with QG18DE)
- **Crank Pattern**: (e.g., 4 teeth per revolution)
- **Cam Pattern**: (e.g., 1 tooth per revolution)
- **ECU You're Using**: (e.g., Speeduino 202310)
- **Current Firmware Version**: 

### Reference Documentation
- Link to trigger pattern documentation (MaxxECU, factory service manual, etc.):
- Oscilloscope captures (if available):

### What You've Tried
- [ ] Checked if ECU firmware already supports this pattern
- [ ] Updated to latest ECU firmware
- [ ] Requested support from ECU firmware project
- [ ] Tried similar existing patterns as workaround

### Additional Context
<!-- Any other information that would be helpful -->

---

## Documentation Reference

For detailed information on trigger pattern support, see:
- [NISSAN_QG18_TRIGGER_SETUP.md](../../docs/NISSAN_QG18_TRIGGER_SETUP.md)
- [FAQ - Trigger Patterns](../../docs/src/faq.md#trigger-patterns)
- [Supported ECUs](../../docs/src/reference/supported-ecus.md#trigger-patterns)
