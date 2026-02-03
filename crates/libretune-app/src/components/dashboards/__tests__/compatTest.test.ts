import { describe, it, expect } from 'vitest';
import { testLibreTuneDashboardCompatibility } from '../compatTest';
import { createLibreTuneDefaultDashboard } from '../LibreTuneDefaultDashboard';
import { DashFile } from '../dashTypes';

describe('Dashboard compatibility checker', () => {
  it('passes for the LibreTune default dashboard', async () => {
    const result = await testLibreTuneDashboardCompatibility([]);
    expect(result.unsupportedPainters).toHaveLength(0);
    expect(result.missingFields).toHaveLength(0);
    expect(result.usesReferenceAssets).toBe(false);
  });

  it('flags unsupported gauge painters', async () => {
    const base = createLibreTuneDefaultDashboard();
    const invalid: DashFile = {
      ...base,
      gauge_cluster: {
        ...base.gauge_cluster,
        components: [
          ...base.gauge_cluster.components,
          {
            Gauge: {
              ...(base.gauge_cluster.components[0] as any).Gauge,
              id: 'unsupported',
              gauge_painter: 'UnknownPainter' as any,
            },
          },
        ],
      },
    };

    const result = await testLibreTuneDashboardCompatibility([invalid]);
    expect(result.unsupportedPainters).toContain('UnknownPainter');
  });
});
