/**
 * TS Indicator Renderer
 * 
 * Renders boolean indicators (warning lights) based on TS IndicatorPainter.
 */

import { TsIndicatorConfig, tsColorToRgba } from '../dashboards/dashTypes';

interface TsIndicatorProps {
  config: TsIndicatorConfig;
  isOn: boolean;
  embeddedImages?: Map<string, string>;
}

export default function TsIndicator({ config, isOn, embeddedImages }: TsIndicatorProps) {
  const backgroundColor = isOn 
    ? tsColorToRgba(config.on_background_color)
    : tsColorToRgba(config.off_background_color);
  
  const textColor = isOn
    ? tsColorToRgba(config.on_text_color)
    : tsColorToRgba(config.off_text_color);

  const text = isOn ? config.on_text : config.off_text;

  // Check for image-based indicator
  const imageName = isOn ? config.on_image_file_name : config.off_image_file_name;
  const imageUrl = imageName && embeddedImages?.get(imageName);

  if (imageUrl) {
    return (
      <div
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          overflow: 'hidden',
        }}
      >
        <img 
          src={imageUrl} 
          alt={text}
          style={{
            maxWidth: '100%',
            maxHeight: '100%',
            objectFit: 'contain',
          }}
        />
      </div>
    );
  }

  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        backgroundColor,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        border: '1px solid rgba(100, 100, 100, 0.5)',
        borderRadius: '2px',
        overflow: 'hidden',
        fontFamily: config.font_family || 'Arial, sans-serif',
        fontStyle: config.italic_font ? 'italic' : 'normal',
        boxSizing: 'border-box',
      }}
    >
      <span
        style={{
          color: textColor,
          fontSize: 'clamp(7px, 1.8vmin, 12px)',
          fontWeight: 500,
          textAlign: 'center',
          padding: '1px 3px',
          lineHeight: 1.15,
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          maxWidth: '100%',
        }}
      >
        {text}
      </span>
    </div>
  );
}
