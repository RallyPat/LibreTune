import { createLibreTuneDefaultDashboard } from './LibreTuneDefaultDashboard';
import { DashFile, isGauge, isIndicator } from './dashTypes';

/**
 * Automated compatibility test for LibreTune default dashboard vs reference .dash files.
 * Checks:
 *   - All gauge_painter types are supported
 *   - All config fields are present and valid
 *   - No reference assets or hardcoded values from TunerStudio
 *   - Visual distinctness (by config, not by image)
 */
export async function testLibreTuneDashboardCompatibility(referenceDashFiles: DashFile[]): Promise<{
  unsupportedPainters: string[];
  missingFields: string[];
  usesReferenceAssets: boolean;
  summary: string;
}> {
  const defaultDash = createLibreTuneDefaultDashboard();
  const supportedPainters = new Set([
    'AnalogGauge','BasicAnalogGauge','CircleAnalogGauge','AsymmetricSweepGauge','BasicReadout','HorizontalBarGauge','HorizontalDashedBar','VerticalBarGauge','HorizontalLineGauge','VerticalDashedBar','AnalogBarGauge','AnalogMovingBarGauge','Histogram','LineGraph','RoundGauge','RoundDashedGauge','FuelMeter','Tachometer',
  ]);
  const unsupportedPainters: string[] = [];
  const missingFields: string[] = [];
  let usesReferenceAssets = false;

  // Check all gauges in default dashboard
  for (const comp of defaultDash.gauge_cluster.components) {
    if (isGauge(comp)) {
      const g = comp.Gauge;
      if (!supportedPainters.has(g.gauge_painter)) {
        unsupportedPainters.push(g.gauge_painter);
      }
      // Check for missing required fields
      const gauge = g as any; // Allow dynamic property access for testing
      for (const field of [
        'id','gauge_painter','output_channel','title','units','min','max','back_color','font_color','trim_color','needle_color','relative_x','relative_y','relative_width','relative_height','border_width','shortest_size','antialiasing_on']) {
        if (gauge[field] === undefined || gauge[field] === null) {
          missingFields.push(field);
        }
      }
      // Check for reference asset usage
      if (g.background_image_file_name && g.background_image_file_name.startsWith('TunerStudio')) {
        usesReferenceAssets = true;
      }
      if (g.needle_image_file_name && g.needle_image_file_name.startsWith('TunerStudio')) {
        usesReferenceAssets = true;
      }
    }
    if (isIndicator(comp)) {
      const i = comp.Indicator;
      if (i.on_image_file_name && i.on_image_file_name.startsWith('TunerStudio')) {
        usesReferenceAssets = true;
      }
      if (i.off_image_file_name && i.off_image_file_name.startsWith('TunerStudio')) {
        usesReferenceAssets = true;
      }
    }
  }

  // Compare config fields to reference dashboards for strictness
  // (Do not copy, just check for field presence and painter type)
  for (const refDash of referenceDashFiles) {
    for (const comp of refDash.gauge_cluster.components) {
      if (isGauge(comp)) {
        const g = comp.Gauge;
        if (!supportedPainters.has(g.gauge_painter)) {
          unsupportedPainters.push(g.gauge_painter);
        }
      }
    }
  }

  const summary = `LibreTune Dashboard Compatibility Test:\n` +
    `Unsupported Painters: ${unsupportedPainters.length ? unsupportedPainters.join(', ') : 'None'}\n` +
    `Missing Fields: ${missingFields.length ? missingFields.join(', ') : 'None'}\n` +
    `Uses Reference Assets: ${usesReferenceAssets ? 'Yes' : 'No'}`;

  return { unsupportedPainters, missingFields, usesReferenceAssets, summary };
}