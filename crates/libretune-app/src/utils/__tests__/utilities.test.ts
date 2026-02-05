import { describe, it, expect } from 'vitest';

/**
 * Frontend Utility Tests
 * Tests for common frontend utilities, helpers, and state management
 */

describe('Value Formatting Utilities', () => {
  it('should format scalar values with 2 decimal places', () => {
    const format = (value: number) => value.toFixed(2);
    expect(format(3.14159)).toBe('3.14');
  });

  it('should handle zero correctly', () => {
    const format = (value: number) => value.toFixed(2);
    expect(format(0)).toBe('0.00');
  });

  it('should handle negative values', () => {
    const format = (value: number) => value.toFixed(2);
    expect(format(-42.5)).toBe('-42.50');
  });

  it('should handle large numbers', () => {
    const format = (value: number) => value.toFixed(2);
    expect(format(99999.999)).toBe('100000.00');
  });

  it('should format array values', () => {
    const values = [1.5, 2.5, 3.5];
    const formatted = values.map(v => v.toFixed(1));
    expect(formatted).toEqual(['1.5', '2.5', '3.5']);
  });
});

describe('Temperature Conversion Display', () => {
  const toF = (c: number) => (c * 9/5 + 32).toFixed(1);
  const toC = (f: number) => ((f - 32) * 5/9).toFixed(1);

  it('should convert 0°C to 32°F', () => {
    expect(toF(0)).toBe('32.0');
  });

  it('should convert 100°C to 212°F', () => {
    expect(toF(100)).toBe('212.0');
  });

  it('should convert 32°F to 0°C', () => {
    expect(toC(32)).toBe('0.0');
  });

  it('should handle negative temperatures', () => {
    expect(toF(-40)).toBe('-40.0');
  });
});

describe('Table Data Bounds Checking', () => {
  const clamp = (value: number, min: number, max: number) => 
    Math.max(min, Math.min(max, value));

  it('should clamp value below min', () => {
    expect(clamp(-10, 0, 100)).toBe(0);
  });

  it('should clamp value above max', () => {
    expect(clamp(150, 0, 100)).toBe(100);
  });

  it('should keep value in range', () => {
    expect(clamp(50, 0, 100)).toBe(50);
  });

  it('should handle float bounds', () => {
    expect(clamp(2.5, 0.0, 2.0)).toBe(2.0);
  });
});

describe('Array Indexing Validation', () => {
  it('should validate matrix index within bounds', () => {
    const rows = 16, cols = 16;
    const isValid = (r: number, c: number) => r >= 0 && r < rows && c >= 0 && c < cols;
    
    expect(isValid(0, 0)).toBe(true);
    expect(isValid(15, 15)).toBe(true);
    expect(isValid(-1, 0)).toBe(false);
    expect(isValid(16, 0)).toBe(false);
  });

  it('should calculate linear index from 2D coords', () => {
    const toLinear = (r: number, c: number, cols: number) => r * cols + c;
    const cols = 16;
    
    expect(toLinear(0, 0, cols)).toBe(0);
    expect(toLinear(0, 15, cols)).toBe(15);
    expect(toLinear(1, 0, cols)).toBe(16);
    expect(toLinear(15, 15, cols)).toBe(255);
  });
});

describe('String Validation', () => {
  it('should validate INI name format', () => {
    const isValid = (name: string) => 
      /^[a-zA-Z0-9_\-\.]+$/i.test(name) && name.length > 0 && name.length <= 255;
    
    expect(isValid('speeduino.ini')).toBe(true);
    expect(isValid('rusEFI_master.ini')).toBe(true);
    expect(isValid('tuner-studio.ini')).toBe(true);
    expect(isValid('')).toBe(false);
    expect(isValid('name with spaces.ini')).toBe(false);
  });

  it('should validate hex color codes', () => {
    const isHex = (color: string) => /^#[0-9A-F]{6}$/i.test(color);
    
    expect(isHex('#FF0000')).toBe(true);
    expect(isHex('#00ff00')).toBe(true);
    expect(isHex('#ABCDEF')).toBe(true);
    expect(isHex('FF0000')).toBe(false);
    expect(isHex('#FF000')).toBe(false);
  });
});

describe('Object Path Navigation', () => {
  it('should safely access nested properties', () => {
    const obj = { a: { b: { c: 42 } } };
    const getPath = (obj: any, path: string) => {
      return path.split('.').reduce((acc, part) => acc?.[part], obj);
    };
    
    expect(getPath(obj, 'a.b.c')).toBe(42);
    expect(getPath(obj, 'a.b')).toEqual({ c: 42 });
    expect(getPath(obj, 'a.x')).toBeUndefined();
    expect(getPath(obj, 'x.y.z')).toBeUndefined();
  });
});

describe('Debounce Helper', () => {
  it('should debounce function calls', (done) => {
    let callCount = 0;
    const debounce = (fn: () => void, delay: number) => {
      let timeoutId: ReturnType<typeof setTimeout>;
      return () => {
        clearTimeout(timeoutId);
        timeoutId = setTimeout(fn, delay);
      };
    };

    const callback = () => callCount++;
    const debounced = debounce(callback, 50);

    debounced();
    debounced();
    debounced();
    
    expect(callCount).toBe(0);
    
    setTimeout(() => {
      expect(callCount).toBe(1);
      done();
    }, 100);
  });
});

describe('Vector Math', () => {
  it('should calculate distance between points', () => {
    const distance = (x1: number, y1: number, x2: number, y2: number) => {
      const dx = x2 - x1;
      const dy = y2 - y1;
      return Math.sqrt(dx * dx + dy * dy);
    };

    expect(distance(0, 0, 3, 4)).toBe(5);
    expect(distance(0, 0, 0, 0)).toBe(0);
  });

  it('should normalize vector', () => {
    const normalize = (x: number, y: number) => {
      const len = Math.sqrt(x * x + y * y);
      return len === 0 ? [0, 0] : [x / len, y / len];
    };

    const [nx, ny] = normalize(3, 4);
    expect(nx).toBeCloseTo(0.6, 5);
    expect(ny).toBeCloseTo(0.8, 5);
  });
});

describe('Color Utilities', () => {
  it('should convert RGB to hex', () => {
    const rgbToHex = (r: number, g: number, b: number) => {
      return '#' + [r, g, b].map(x => x.toString(16).padStart(2, '0').toUpperCase()).join('');
    };

    expect(rgbToHex(255, 0, 0)).toBe('#FF0000');
    expect(rgbToHex(0, 255, 0)).toBe('#00FF00');
    expect(rgbToHex(0, 0, 255)).toBe('#0000FF');
  });

  it('should convert hex to RGB', () => {
    const hexToRgb = (hex: string) => {
      const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
      return result ? [parseInt(result[1], 16), parseInt(result[2], 16), parseInt(result[3], 16)] : null;
    };

    expect(hexToRgb('#FF0000')).toEqual([255, 0, 0]);
    expect(hexToRgb('#00FF00')).toEqual([0, 255, 0]);
    expect(hexToRgb('0000FF')).toEqual([0, 0, 255]); // Should also work without #
  });
});

describe('Time Formatting', () => {
  it('should format milliseconds as time string', () => {
    const formatTime = (ms: number) => {
      const seconds = Math.floor(ms / 1000);
      const minutes = Math.floor(seconds / 60);
      const hours = Math.floor(minutes / 60);
      
      if (hours > 0) return `${hours}h ${minutes % 60}m`;
      if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
      return `${seconds}s`;
    };

    expect(formatTime(0)).toBe('0s');
    expect(formatTime(1000)).toBe('1s');
    expect(formatTime(60000)).toBe('1m 0s');
    expect(formatTime(3600000)).toBe('1h 0m');
  });
});

describe('Percentage Calculations', () => {
  it('should calculate percentage change', () => {
    const percentChange = (from: number, to: number) => 
      Math.round((to - from) / from * 10000) / 100;

    expect(percentChange(100, 110)).toBe(10);
    expect(percentChange(100, 90)).toBe(-10);
    expect(percentChange(50, 100)).toBe(100);
  });

  it('should calculate relative percentage', () => {
    const relativePercent = (current: number, min: number, max: number) =>
      Math.round((current - min) / (max - min) * 100);

    expect(relativePercent(50, 0, 100)).toBe(50);
    expect(relativePercent(0, 0, 100)).toBe(0);
    expect(relativePercent(100, 0, 100)).toBe(100);
  });
});
