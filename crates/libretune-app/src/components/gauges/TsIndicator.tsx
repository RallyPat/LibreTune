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
  const getFontFamily = (fontFamily?: string): string => {
    const webSafeStacks: Record<string, string> = {
      Arial: 'Arial, Helvetica, sans-serif',
      'Arial Black': '"Arial Black", Gadget, sans-serif',
      Verdana: 'Verdana, Geneva, sans-serif',
      Tahoma: 'Tahoma, Geneva, sans-serif',
      'Trebuchet MS': '"Trebuchet MS", Helvetica, sans-serif',
      Georgia: 'Georgia, serif',
      'Times New Roman': '"Times New Roman", Times, serif',
      'Courier New': '"Courier New", Courier, monospace',
      Consolas: 'Consolas, Monaco, "Lucida Console", monospace',
      Monaco: 'Monaco, Consolas, monospace',
    };

    const defaultStack = 'Arial, Helvetica, sans-serif';

    if (!fontFamily) {
      return defaultStack;
    }

    if (webSafeStacks[fontFamily]) {
      return webSafeStacks[fontFamily];
    }

    return `"${fontFamily}", Arial, Helvetica, sans-serif`;
  };

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
            imageRendering: config.antialiasing_on ? 'auto' : 'pixelated',
          }}
        />
      </div>
    );
  }

  if (config.indicator_painter === 'BulbIndicator') {
    const bulbColor = isOn ? backgroundColor : 'rgba(20, 20, 20, 0.9)';
    const glowColor = isOn ? backgroundColor : 'transparent';

    return (
      <div
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <div
          style={{
            width: '80%',
            height: '80%',
            borderRadius: '50%',
            background: `radial-gradient(circle at 30% 30%, rgba(255, 255, 255, 0.7), ${bulbColor})`,
            boxShadow: isOn ? `0 0 12px ${glowColor}` : 'inset 0 0 6px rgba(0, 0, 0, 0.6)',
            border: '1px solid rgba(255, 255, 255, 0.2)',
          }}
          aria-label={text}
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
        fontFamily: getFontFamily(config.font_family),
        fontStyle: config.italic_font ? 'italic' : 'normal',
        textRendering: config.antialiasing_on ? 'auto' : 'optimizeSpeed',
        WebkitFontSmoothing: config.antialiasing_on ? 'antialiased' : 'none',
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
