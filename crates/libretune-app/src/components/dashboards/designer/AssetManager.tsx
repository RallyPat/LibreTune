/**
 * AssetManager — Plan v2 / D-7e.
 *
 * Lets the user inspect and curate the `embedded_images` array on the
 * primary cluster: add an image from local disk (base64-encoded), preview
 * it, copy its `image_id` for use in gauge property fields, and delete
 * orphaned assets.
 *
 * Pure-frontend; no new backend command — the dash file save path
 * already persists `embedded_images` via `save_dash_file`.
 */

import { useRef } from 'react';
import { DashFile, EmbeddedImage, ResourceType } from '../dashTypes';

interface Props {
  dashFile: DashFile;
  onChange: (file: DashFile) => void;
}

function inferResourceType(name: string): ResourceType {
  const lower = name.toLowerCase();
  if (lower.endsWith('.gif')) return 'Gif';
  if (lower.endsWith('.ttf')) return 'Ttf';
  return 'Png';
}

function strip64(dataUrl: string): string {
  const i = dataUrl.indexOf(',');
  return i >= 0 ? dataUrl.slice(i + 1) : dataUrl;
}

export default function AssetManager({ dashFile, onChange }: Props) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const cluster = dashFile.gauge_cluster;
  const images = cluster.embedded_images;

  const replaceImages = (next: EmbeddedImage[]) => {
    onChange({
      ...dashFile,
      gauge_cluster: { ...cluster, embedded_images: next },
    });
  };

  const addFiles = async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    const additions: EmbeddedImage[] = [];
    for (const f of Array.from(files)) {
      try {
        const dataUrl: string = await new Promise((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(String(reader.result));
          reader.onerror = () => reject(reader.error);
          reader.readAsDataURL(f);
        });
        additions.push({
          file_name: f.name,
          image_id: f.name,
          resource_type: inferResourceType(f.name),
          data: strip64(dataUrl),
        });
      } catch (err) {
        console.error('AssetManager: failed to read', f.name, err);
      }
    }
    if (additions.length) replaceImages([...images, ...additions]);
  };

  const remove = (idx: number) => {
    const next = [...images];
    next.splice(idx, 1);
    replaceImages(next);
  };

  const renameId = (idx: number, newId: string) => {
    if (!newId.trim()) return;
    const next = [...images];
    next[idx] = { ...next[idx], image_id: newId.trim() };
    replaceImages(next);
  };

  return (
    <div className="asset-manager">
      <h4>Embedded Assets</h4>
      <p className="asset-hint">
        Reference an asset from a gauge by setting its background or needle
        image to the asset's <code>image_id</code>.
      </p>
      <div className="asset-actions">
        <button type="button" onClick={() => fileInputRef.current?.click()}>
          + Add Image / Font
        </button>
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept=".png,.gif,.ttf,image/png,image/gif,font/ttf"
          style={{ display: 'none' }}
          onChange={(e) => {
            void addFiles(e.target.files);
            e.target.value = '';
          }}
        />
      </div>
      {images.length === 0 ? (
        <p className="no-selection">No embedded assets</p>
      ) : (
        <ul className="asset-list">
          {images.map((img, i) => {
            const isImage = img.resource_type !== 'Ttf';
            const mime =
              img.resource_type === 'Gif'
                ? 'image/gif'
                : img.resource_type === 'Png'
                  ? 'image/png'
                  : 'font/ttf';
            const src = isImage ? `data:${mime};base64,${img.data}` : '';
            return (
              <li key={`${img.image_id}-${i}`} className="asset-row">
                {isImage ? (
                  <img src={src} alt={img.image_id} className="asset-thumb" />
                ) : (
                  <span className="asset-thumb font-thumb">Aa</span>
                )}
                <div className="asset-meta">
                  <input
                    type="text"
                    value={img.image_id}
                    onChange={(e) => renameId(i, e.target.value)}
                    title="Asset ID (referenced from gauge image fields)"
                  />
                  <span className="asset-filename">{img.file_name}</span>
                  <span className="asset-kind">
                    {img.resource_type} · {Math.round((img.data.length * 3) / 4 / 1024)} KB
                  </span>
                </div>
                <button
                  type="button"
                  className="asset-delete"
                  title="Delete asset"
                  onClick={() => remove(i)}
                >✕</button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
