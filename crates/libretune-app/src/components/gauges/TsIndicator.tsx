/**
 * TunerStudio Indicator Renderer
 * 
 * Renders boolean indicators (warning lights) based on TunerStudio's IndicatorPainter.
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
        fontFamily: config.font_family || 'sans-serif',
        fontStyle: config.italic_font ? 'italic' : 'normal',
      }}
    >
      <span
        style={{
          color: textColor,
          fontSize: 'clamp(8px, 80%, 14px)',
          fontWeight: 'bold',
          textAlign: 'center',
          padding: '2px',
          lineHeight: 1.1,
          wordBreak: 'break-word',
        }}
      >
        {text}
      </span>
    </div>
  );
}
