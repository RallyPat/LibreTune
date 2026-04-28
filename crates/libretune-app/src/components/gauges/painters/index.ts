/**
 * Painter registry — central registration of every per-painter
 * module. Import this file once (e.g. from `TsGauge.tsx`) to populate
 * `painterRegistry`. Adding a new painter is a one-line addition here
 * plus deletion of the corresponding inline closure.
 */

import { registerPainter } from './types';
import { basicReadoutPainter } from './basicReadout';
import { horizontalBarPainter } from './horizontalBar';
import { verticalBarPainter } from './verticalBar';
import { horizontalLinePainter } from './horizontalLine';
import { verticalDashedBarPainter } from './verticalDashedBar';
import { horizontalDashedBarPainter } from './horizontalDashedBar';
import { histogramPainter } from './histogram';
import { lineGraphPainter } from './lineGraph';
import { roundGaugePainter } from './roundGauge';
import { roundDashedGaugePainter } from './roundDashedGauge';

export { painterRegistry, type Painter, type PainterContext } from './types';

let registered = false;

/**
 * Idempotently register all migrated painters. Safe to call multiple
 * times (e.g. once per `TsGauge` instance) — only the first call
 * actually mutates the registry.
 */
export function ensurePaintersRegistered(): void {
  if (registered) return;
  registered = true;
  registerPainter('BasicReadout', basicReadoutPainter);
  registerPainter('HorizontalBarGauge', horizontalBarPainter);
  registerPainter('VerticalBarGauge', verticalBarPainter);
  registerPainter('HorizontalLineGauge', horizontalLinePainter);
  registerPainter('VerticalDashedBar', verticalDashedBarPainter);
  registerPainter('HorizontalDashedBar', horizontalDashedBarPainter);
  registerPainter('Histogram', histogramPainter);
  registerPainter('LineGraph', lineGraphPainter);
  registerPainter('RoundGauge', roundGaugePainter);
  registerPainter('RoundDashedGauge', roundDashedGaugePainter);
}
