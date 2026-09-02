//! The simulator's hidden "true VE" surface, and the decode of the *table*
//! VE the tune currently holds in page memory.
//!
//! Together these close the AutoTune demo loop:
//!
//! ```text
//! afr = afr_target × true_ve / current_ve
//! ```
//!
//! Where the loaded `veTable` is wrong, `current_ve` differs from
//! [`true_ve`] and the simulated "measured" AFR drifts away from the target
//! — exactly the error surface AutoTune has to flatten. Correcting a cell to
//! `VE_new = VE_old × afr / target` converges on `true_ve` in one step, by
//! construction, so the demo shows AutoTune visibly working.
//!
//! Row-major convention: a `zBins` array decodes as row = Y bins (load),
//! col = X bins (rpm) — `ve[row * cols + col]`. That matches the real
//! Speeduino/TunerStudio veTable layout, where each row is one load bin
//! across all RPM columns.

use crate::ecu::EcuMemory;
use crate::ini::{Constant, EcuDefinition, Endianness, Shape};

/// Decoded `veTable` context: physical axis bins plus physical cell values,
/// refreshed from the memory image on each engine tick.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VeContext {
    pub(crate) rpm_bins: Vec<f64>,
    pub(crate) load_bins: Vec<f64>,
    /// Row-major, row = `load_bins` index, col = `rpm_bins` index.
    pub(crate) ve: Vec<f64>,
}

impl VeContext {
    /// Whether this context describes a real tune rather than blank memory.
    ///
    /// An all-zero page decodes into a structurally valid but meaningless
    /// context: flat axes and 0 % cells. Interpolating it yields a VE the
    /// engine model would divide by, producing an AFR no consumer can use,
    /// so it is rejected at the source instead.
    pub(crate) fn is_usable(&self) -> bool {
        let ascending = |bins: &[f64]| bins.len() >= 2 && bins.windows(2).all(|w| w[1] > w[0]);
        ascending(&self.rpm_bins)
            && ascending(&self.load_bins)
            && self.ve.len() == self.rpm_bins.len() * self.load_bins.len()
            && self.ve.iter().any(|v| *v > 0.0)
    }

    /// Bilinear current-VE lookup, clamped to the bin range.
    ///
    /// `None` when the bins are empty or `ve`'s length doesn't match
    /// `rpm_bins.len() * load_bins.len()` — a shape mismatch must never
    /// index out of bounds.
    pub(crate) fn current_ve(&self, rpm: f64, load_kpa: f64) -> Option<f64> {
        let nx = self.rpm_bins.len();
        let ny = self.load_bins.len();
        if nx == 0 || ny == 0 || self.ve.len() != nx * ny {
            return None;
        }
        let (x0, fx) = segment(&self.rpm_bins, rpm);
        let (y0, fy) = segment(&self.load_bins, load_kpa);
        let x1 = (x0 + 1).min(nx - 1);
        let y1 = (y0 + 1).min(ny - 1);
        let cell = |y: usize, x: usize| self.ve[y * nx + x];
        let top = cell(y0, x0) * (1.0 - fx) + cell(y0, x1) * fx;
        let bottom = cell(y1, x0) * (1.0 - fx) + cell(y1, x1) * fx;
        Some(top * (1.0 - fy) + bottom * fy)
    }
}

/// The simulator's hidden "true VE" surface — what the virtual engine
/// actually needs, regardless of what the loaded `veTable` says.
///
/// Deterministic and cell-dependent, so the lean error grows with load and
/// rpm and AutoTune has a real gradient to chase.
pub(crate) fn true_ve(rpm: f64, load_kpa: f64) -> f64 {
    let unclamped = 40.0 + 25.0 * (load_kpa / 100.0) + 15.0 * (rpm / 6000.0);
    unclamped.clamp(20.0, 110.0)
}

/// Locate `v`'s bracketing segment in ascending `bins`: the lower index and
/// the fractional position within that segment. Out-of-range values clamp to
/// the first/last segment with fraction 0.0/1.0 — never extrapolates.
fn segment(bins: &[f64], v: f64) -> (usize, f64) {
    let last = bins.len().saturating_sub(1);
    if last == 0 {
        return (0, 0.0);
    }
    if v <= bins[0] {
        return (0, 0.0);
    }
    if v >= bins[last] {
        return (last - 1, 1.0);
    }
    for i in 0..last {
        let (lo, hi) = (bins[i], bins[i + 1]);
        if v >= lo && v <= hi {
            let span = hi - lo;
            let frac = if span > 0.0 { (v - lo) / span } else { 0.0 };
            return (i, frac);
        }
    }
    (last - 1, 1.0)
}

/// Resolve and decode the VE table the INI's `[VeAnalyze]` section points
/// at, straight out of the simulator's page memory.
///
/// Fails open (`None`) at every step — no `[VeAnalyze]`, an undeclared
/// table, a missing bins/map constant, or a page read shorter than the
/// declared shape all yield no context rather than a panic. Called once per
/// engine tick; the lookups are small map hits, cheap enough not to cache.
pub(crate) fn ve_context(def: &EcuDefinition, memory: &EcuMemory) -> Option<VeContext> {
    let table_name = &def.ve_analyze.as_ref()?.ve_table_name;
    let table = def.tables.get(table_name).or_else(|| {
        def.table_map_to_name
            .get(table_name)
            .and_then(|resolved| def.tables.get(resolved))
    })?;

    let rpm_bins = decode_constant(def, memory, &table.x_bins)?;
    let load_bins = decode_constant(def, memory, table.y_bins.as_ref()?)?;
    let ve = decode_constant(def, memory, &table.map)?;
    let ctx = VeContext {
        rpm_bins,
        load_bins,
        ve,
    };
    ctx.is_usable().then_some(ctx)
}

/// Overwrite the `[VeAnalyze]` veTable in `memory` with a plausible starting
/// tune: ascending axes and a VE surface deliberately below the engine's
/// hidden truth.
///
/// A freshly built [`EcuMemory`] is all zeroes, and a zeroed veTable is not
/// a tune — its axes are degenerate, its cells are 0 %, and the AFR the
/// model would report from it is meaningless. Seeding gives the AutoTune
/// demo a real error surface to flatten instead of a division by nothing.
///
/// Returns whether a table was found and written.
pub(crate) fn seed_ve_table(def: &EcuDefinition, memory: &mut EcuMemory) -> bool {
    let Some(analyze) = def.ve_analyze.as_ref() else {
        return false;
    };
    let table = match def.tables.get(&analyze.ve_table_name).or_else(|| {
        def.table_map_to_name
            .get(&analyze.ve_table_name)
            .and_then(|resolved| def.tables.get(resolved))
    }) {
        Some(table) => table,
        None => return false,
    };
    let Some(y_bins) = table.y_bins.as_ref() else {
        return false;
    };

    let rpm = spread(count_of(def, &table.x_bins), SEED_RPM_MIN, SEED_RPM_MAX);
    let load = spread(count_of(def, y_bins), SEED_LOAD_MIN, SEED_LOAD_MAX);
    if rpm.len() < 2 || load.len() < 2 {
        return false;
    }
    // Deliberately short of the truth, so the first AutoTune pass has
    // something visible to correct.
    let cells: Vec<f64> = load
        .iter()
        .flat_map(|l| rpm.iter().map(move |r| true_ve(*r, *l) * SEED_VE_ERROR))
        .collect();

    encode_constant(def, memory, &table.x_bins, &rpm)
        && encode_constant(def, memory, y_bins, &load)
        && encode_constant(def, memory, &table.map, &cells)
}

/// Seeded axis range and the deliberate VE error baked into the seed.
const SEED_RPM_MIN: f64 = 500.0;
const SEED_RPM_MAX: f64 = 7000.0;
const SEED_LOAD_MIN: f64 = 20.0;
const SEED_LOAD_MAX: f64 = 100.0;
const SEED_VE_ERROR: f64 = 0.85;

/// `n` values evenly spanning `[lo, hi]`.
fn spread(n: usize, lo: f64, hi: f64) -> Vec<f64> {
    if n < 2 {
        return Vec::new();
    }
    let step = (hi - lo) / (n - 1) as f64;
    (0..n).map(|i| lo + step * i as f64).collect()
}

fn count_of(def: &EcuDefinition, name: &str) -> usize {
    def.constants
        .get(name)
        .map(|c| element_count(&c.shape))
        .unwrap_or(0)
}

/// Inverse of [`decode_constant`]: write physical values back as raw bytes.
fn encode_constant(
    def: &EcuDefinition,
    memory: &mut EcuMemory,
    name: &str,
    values: &[f64],
) -> bool {
    let Some(constant) = def.constants.get(name) else {
        return false;
    };
    let width = constant.data_type.size_bytes();
    if width == 0 || element_count(&constant.shape) != values.len() {
        return false;
    }
    let endian = constant.endianness_override.unwrap_or(def.endianness);
    let mut bytes = vec![0u8; values.len() * width];
    for (i, physical) in values.iter().enumerate() {
        let raw = if constant.scale != 0.0 {
            (physical - constant.translate) / constant.scale
        } else {
            0.0
        };
        let raw = match constant.data_type.raw_range_bounds() {
            Some((lo, hi)) => raw.clamp(lo, hi),
            None => raw,
        };
        constant
            .data_type
            .write_to_bytes(&mut bytes, i * width, raw, endian);
    }
    memory.write_bytes(constant.page, constant.offset, &bytes)
}

/// Decode a named constant's current raw bytes into physical values,
/// preserving the array's declared row-major order.
///
/// Uses `DataType::read_from_bytes` — the same decode the realtime path
/// runs — so endianness handling cannot drift between the two. Applies the
/// INI's `physical = raw * scale + translate` per element.
fn decode_constant(def: &EcuDefinition, memory: &EcuMemory, name: &str) -> Option<Vec<f64>> {
    let constant = def.constants.get(name)?;
    let count = element_count(&constant.shape);
    if count == 0 {
        return None;
    }
    let width = constant.data_type.size_bytes();
    if width == 0 {
        return None;
    }
    let total = u16::try_from(count.checked_mul(width)?).ok()?;
    let bytes = memory.read_bytes(constant.page, constant.offset, total)?;
    let endian = constant.endianness_override.unwrap_or(def.endianness);
    (0..count)
        .map(|i| decode_element(constant, bytes, i * width, endian))
        .collect()
}

fn decode_element(
    constant: &Constant,
    bytes: &[u8],
    offset: usize,
    endian: Endianness,
) -> Option<f64> {
    let raw = constant.data_type.read_from_bytes(bytes, offset, endian)?;
    Some(raw * constant.scale + constant.translate)
}

fn element_count(shape: &Shape) -> usize {
    match shape {
        Shape::Scalar => 1,
        Shape::Array1D(n) => *n,
        Shape::Array2D { rows, cols } => rows * cols,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ini::TableDefinition;
    use crate::ini::{DataType, VeAnalyzeConfig};

    fn context() -> VeContext {
        VeContext {
            rpm_bins: vec![1000.0, 2000.0],
            load_bins: vec![40.0, 80.0],
            // row-major: row = load bin, col = rpm bin
            ve: vec![50.0, 60.0, 70.0, 80.0],
        }
    }

    #[test]
    fn current_ve_interpolates_bilinearly_between_the_four_surrounding_cells() {
        // Dead centre of the grid averages all four corners.
        assert_eq!(context().current_ve(1500.0, 60.0), Some(65.0));
        // On a bin line it reduces to a 1D interpolation.
        assert_eq!(context().current_ve(1000.0, 60.0), Some(60.0));
        assert_eq!(context().current_ve(1500.0, 40.0), Some(55.0));
    }

    #[test]
    fn current_ve_clamps_instead_of_extrapolating_past_the_bins() {
        let ctx = context();
        assert_eq!(ctx.current_ve(0.0, 0.0), Some(50.0), "below the first bin");
        assert_eq!(
            ctx.current_ve(9000.0, 300.0),
            Some(80.0),
            "above the last bin"
        );
    }

    #[test]
    fn current_ve_refuses_a_grid_whose_cell_count_contradicts_its_bins() {
        let mut ctx = context();
        ctx.ve.pop();
        assert_eq!(ctx.current_ve(1500.0, 60.0), None);
    }

    #[test]
    fn true_ve_rises_with_load_and_rpm_and_stays_inside_its_clamp() {
        assert!(
            true_ve(3000.0, 80.0) > true_ve(3000.0, 40.0),
            "load raises VE"
        );
        assert!(
            true_ve(5000.0, 60.0) > true_ve(1000.0, 60.0),
            "rpm raises VE"
        );
        assert_eq!(true_ve(-1e9, -1e9), 20.0);
        assert_eq!(true_ve(1e9, 1e9), 110.0);
    }

    #[test]
    fn ve_context_decodes_the_ve_analyze_table_out_of_page_memory() {
        // Arrange: a definition whose VeAnalyze points at a 2x2 veTable, with
        // a scale that makes a raw/physical mix-up visible.
        let mut def = EcuDefinition::default();
        def.endianness = Endianness::Little;
        def.page_sizes = vec![64];
        def.n_pages = 1;
        def.ve_analyze = Some(VeAnalyzeConfig {
            ve_table_name: "veTable".to_string(),
            ..Default::default()
        });

        let mut table = TableDefinition::default();
        table.name = "veTable".to_string();
        table.x_bins = "rpmBins".to_string();
        table.y_bins = Some("loadBins".to_string());
        table.map = "veTableCells".to_string();
        def.tables.insert("veTable".to_string(), table);

        let constant = |offset: u16, shape: Shape, scale: f64| Constant {
            page: 0,
            offset,
            data_type: DataType::U08,
            shape,
            scale,
            translate: 0.0,
            ..Default::default()
        };
        def.constants
            .insert("rpmBins".to_string(), constant(0, Shape::Array1D(2), 100.0));
        def.constants
            .insert("loadBins".to_string(), constant(2, Shape::Array1D(2), 1.0));
        def.constants.insert(
            "veTableCells".to_string(),
            constant(4, Shape::Array2D { rows: 2, cols: 2 }, 1.0),
        );

        let mut memory = EcuMemory::from_definition(&def);
        memory.write_bytes(0, 0, &[10, 20]); // rpm bins: 1000, 2000 after scale
        memory.write_bytes(0, 2, &[40, 80]); // load bins
        memory.write_bytes(0, 4, &[50, 60, 70, 80]); // cells

        // Act
        let ctx = ve_context(&def, &memory).expect("definition and memory are complete");

        // Assert
        assert_eq!(ctx.rpm_bins, vec![1000.0, 2000.0], "scale must be applied");
        assert_eq!(ctx.load_bins, vec![40.0, 80.0]);
        assert_eq!(ctx.ve, vec![50.0, 60.0, 70.0, 80.0]);
        assert_eq!(ctx.current_ve(1500.0, 60.0), Some(65.0));
    }

    #[test]
    fn a_blank_page_is_refused_rather_than_read_as_a_flat_zero_tune() {
        // Zeroed memory decodes into a structurally valid context whose axes
        // are degenerate and whose cells are 0 %. Accepting it made the
        // engine report an AFR no consumer can use.
        let ctx = VeContext {
            rpm_bins: vec![0.0, 0.0],
            load_bins: vec![0.0, 0.0],
            ve: vec![0.0; 4],
        };
        assert!(!ctx.is_usable(), "flat axes and empty cells are not a tune");

        let ascending_but_empty = VeContext {
            rpm_bins: vec![1000.0, 2000.0],
            load_bins: vec![40.0, 80.0],
            ve: vec![0.0; 4],
        };
        assert!(
            !ascending_but_empty.is_usable(),
            "an all-zero surface is still not a tune"
        );

        assert!(context().is_usable(), "a real table must be accepted");
    }

    #[test]
    fn seeding_turns_blank_memory_into_a_context_the_model_can_use() {
        let def = seedable_definition();
        let mut memory = EcuMemory::from_definition(&def);
        assert_eq!(
            ve_context(&def, &memory),
            None,
            "blank memory must not pass as a tune"
        );

        assert!(seed_ve_table(&def, &mut memory), "the table is seeded");

        let ctx = ve_context(&def, &memory).expect("the seeded table is usable");
        assert!(
            ctx.rpm_bins.windows(2).all(|w| w[1] > w[0]),
            "seeded rpm axis must ascend: {:?}",
            ctx.rpm_bins
        );
        let ve = ctx
            .current_ve(3000.0, 60.0)
            .expect("the seeded grid interpolates");
        assert!(
            (10.0..110.0).contains(&ve),
            "seeded VE must be a plausible percentage, got {ve}"
        );
        assert!(
            ve < true_ve(3000.0, 60.0),
            "the seed must sit below the truth, or AutoTune has nothing to correct"
        );
    }

    /// A definition whose veTable is large enough to seed and read back.
    fn seedable_definition() -> EcuDefinition {
        let mut def = EcuDefinition::default();
        def.endianness = Endianness::Little;
        def.page_sizes = vec![256];
        def.n_pages = 1;
        def.ve_analyze = Some(VeAnalyzeConfig {
            ve_table_name: "veTable".to_string(),
            ..Default::default()
        });

        let mut table = TableDefinition::default();
        table.name = "veTable".to_string();
        table.x_bins = "rpmBins".to_string();
        table.y_bins = Some("loadBins".to_string());
        table.map = "veTableCells".to_string();
        def.tables.insert("veTable".to_string(), table);

        let constant = |offset: u16, shape: Shape, scale: f64| Constant {
            page: 0,
            offset,
            data_type: DataType::U08,
            shape,
            scale,
            translate: 0.0,
            ..Default::default()
        };
        def.constants
            .insert("rpmBins".to_string(), constant(0, Shape::Array1D(4), 100.0));
        def.constants
            .insert("loadBins".to_string(), constant(4, Shape::Array1D(4), 1.0));
        def.constants.insert(
            "veTableCells".to_string(),
            constant(8, Shape::Array2D { rows: 4, cols: 4 }, 1.0),
        );
        def
    }

    #[test]
    fn ve_context_fails_open_when_the_ini_declares_no_ve_analyze() {
        let def = EcuDefinition::default();
        let memory = EcuMemory::from_definition(&def);
        assert_eq!(ve_context(&def, &memory), None);
    }
}
