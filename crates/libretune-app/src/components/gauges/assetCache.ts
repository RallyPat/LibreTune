/**
 * Module-level cache for embedded fonts and images shared across all
 * `<TsGauge>` instances. Loading the same data-URI font or PNG twice
 * is wasteful, so we keep a process-global registry keyed by the
 * embedded asset id.
 */

/** Set of font IDs that have been registered with `document.fonts`. */
const loadedFonts = new Set<string>();

/** Cache of decoded `HTMLImageElement` objects keyed by embedded image id. */
const loadedImages = new Map<string, HTMLImageElement>();

/** Look up a previously-loaded embedded image. */
export function getEmbeddedImage(name: string | null | undefined): HTMLImageElement | null {
  if (!name) return null;
  return loadedImages.get(name) || null;
}

/** Was the given embedded font successfully registered? */
export function isFontLoaded(id: string): boolean {
  return loadedFonts.has(id);
}

/**
 * Load every embedded asset in `embeddedImages` (a map of
 * id → data-URI). Fonts are added to `document.fonts` and images are
 * decoded into `HTMLImageElement`s. Subsequent calls with the same ids
 * are no-ops thanks to the module-level caches.
 */
export async function loadEmbeddedAssets(
  embeddedImages: Map<string, string>,
): Promise<void> {
  const loadPromises: Promise<void>[] = [];

  for (const [id, dataUrl] of embeddedImages.entries()) {
    if (dataUrl.startsWith('data:font/ttf') && !loadedFonts.has(id)) {
      loadPromises.push(
        (async () => {
          try {
            const fontFace = new FontFace(id, `url(${dataUrl})`);
            await fontFace.load();
            document.fonts.add(fontFace);
            loadedFonts.add(id);
          } catch (e) {
            console.warn(`Failed to load embedded font ${id}:`, e);
          }
        })(),
      );
    }

    if (
      (dataUrl.startsWith('data:image/png') || dataUrl.startsWith('data:image/gif')) &&
      !loadedImages.has(id)
    ) {
      loadPromises.push(
        new Promise<void>((resolve) => {
          const img = new Image();
          img.onload = () => {
            loadedImages.set(id, img);
            resolve();
          };
          img.onerror = () => {
            console.warn(`Failed to load embedded image ${id}`);
            resolve();
          };
          img.src = dataUrl;
        }),
      );
    }
  }

  await Promise.all(loadPromises);
}
