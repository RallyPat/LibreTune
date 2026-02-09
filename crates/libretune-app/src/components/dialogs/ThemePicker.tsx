import { THEME_INFO, ThemeName } from '../../themes';
import './ThemePicker.css';

interface ThemePickerProps {
  selectedTheme: ThemeName;
  onChange: (theme: ThemeName) => void;
}

/**
 * ThemePicker Component
 * 
 * Visual theme selector showing preview cards for each available theme.
 * Each card displays the theme name and a color preview using the theme's
 * background, primary, and accent colors.
 */
export default function ThemePicker({ selectedTheme, onChange }: ThemePickerProps) {
  const themes = Object.entries(THEME_INFO) as [ThemeName, typeof THEME_INFO[ThemeName]][];

  return (
    <div className="theme-picker">
      {themes.map(([themeKey, theme]) => (
        <button
          key={themeKey}
          type="button"
          className={`theme-card ${selectedTheme === themeKey ? 'selected' : ''}`}
          onClick={() => onChange(themeKey)}
          title={`Switch to ${theme.label} theme`}
        >
          <div className="theme-preview">
            <div className="theme-preview-bg" style={{ backgroundColor: theme.bg }}>
              <div className="theme-preview-primary" style={{ backgroundColor: theme.primary }} />
              <div className="theme-preview-accent" style={{ backgroundColor: theme.accent }} />
            </div>
          </div>
          <div className="theme-name">{theme.label}</div>
          {selectedTheme === themeKey && (
            <div className="theme-checkmark">✓</div>
          )}
        </button>
      ))}
    </div>
  );
}
