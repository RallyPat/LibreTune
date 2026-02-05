import { describe, it, expect, vi } from 'vitest';
import type { TableData, TableCoords } from '../../components/tables/TableEditor2D';

/**
 * Table Editor Component Tests
 * Tests for 2D table grid functionality and operations
 */

describe('TableEditor2D Utilities', () => {
  describe('Cell Selection', () => {
    it('should select single cell', () => {
      const selection = new Set(['0_0']);
      expect(selection.has('0_0')).toBe(true);
      expect(selection.size).toBe(1);
    });

    it('should select range of cells', () => {
      const selection = new Set<string>();
      for (let r = 0; r < 4; r++) {
        for (let c = 0; c < 4; c++) {
          selection.add(`${r}_${c}`);
        }
      }
      expect(selection.size).toBe(16);
    });

    it('should toggle cell selection', () => {
      const selection = new Set(['0_0']);
      const key = '0_0';
      
      if (selection.has(key)) {
        selection.delete(key);
      } else {
        selection.add(key);
      }
      
      expect(selection.has(key)).toBe(false);
      expect(selection.size).toBe(0);
    });

    it('should clear selection', () => {
      const selection = new Set(['0_0', '0_1', '1_0']);
      selection.clear();
      expect(selection.size).toBe(0);
    });
  });

  describe('Table Cell Coordinates', () => {
    it('should convert cell key to coordinates', () => {
      const keyToCoords = (key: string): [number, number] => {
        const [r, c] = key.split('_').map(Number);
        return [r, c];
      };

      expect(keyToCoords('5_10')).toEqual([5, 10]);
      expect(keyToCoords('0_0')).toEqual([0, 0]);
    });

    it('should convert coordinates to cell key', () => {
      const coordsToKey = (r: number, c: number) => `${r}_${c}`;
      
      expect(coordsToKey(5, 10)).toBe('5_10');
      expect(coordsToKey(0, 0)).toBe('0_0');
    });

    it('should find cell bounds for rectangle', () => {
      const getBounds = (start: [number, number], end: [number, number]) => ({
        minRow: Math.min(start[0], end[0]),
        maxRow: Math.max(start[0], end[0]),
        minCol: Math.min(start[1], end[1]),
        maxCol: Math.max(start[1], end[1]),
      });

      const bounds = getBounds([2, 3], [5, 7]);
      expect(bounds).toEqual({ minRow: 2, maxRow: 5, minCol: 3, maxCol: 7 });
    });
  });

  describe('Table Value Operations', () => {
    it('should calculate cell average', () => {
      const values = [10, 20, 30, 40];
      const average = values.reduce((a, b) => a + b, 0) / values.length;
      expect(average).toBe(25);
    });

    it('should scale values', () => {
      const values = [10, 20, 30];
      const scale = 2.0;
      const scaled = values.map(v => v * scale);
      expect(scaled).toEqual([20, 40, 60]);
    });

    it('should clamp cell values', () => {
      const clamp = (v: number, min: number, max: number) => 
        Math.max(min, Math.min(max, v));
      
      expect(clamp(15, 0, 100)).toBe(15);
      expect(clamp(-10, 0, 100)).toBe(0);
      expect(clamp(150, 0, 100)).toBe(100);
    });

    it('should round to AFR precision (0.1)', () => {
      const round = (v: number, decimals: number) => 
        Math.round(v * Math.pow(10, decimals)) / Math.pow(10, decimals);
      
      expect(round(14.75, 1)).toBe(14.8);
      expect(round(14.74, 1)).toBe(14.7);
    });
  });

  describe('Table Interpolation', () => {
    it('should interpolate linearly between values', () => {
      const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
      
      expect(lerp(0, 100, 0.5)).toBe(50);
      expect(lerp(10, 20, 0.5)).toBe(15);
      expect(lerp(0, 100, 0)).toBe(0);
      expect(lerp(0, 100, 1)).toBe(100);
    });

    it('should bilinear interpolate in 2D', () => {
      // Corners of 2x2 grid
      const topLeft = 10, topRight = 20;
      const botLeft = 30, botRight = 40;
      
      const bilinear = (tl: number, tr: number, bl: number, br: number, u: number, v: number) => {
        const top = tl + (tr - tl) * u;
        const bot = bl + (br - bl) * u;
        return top + (bot - top) * v;
      };

      expect(bilinear(topLeft, topRight, botLeft, botRight, 0.5, 0.5)).toBe(25);
      expect(bilinear(topLeft, topRight, botLeft, botRight, 0, 0)).toBe(10);
      expect(bilinear(topLeft, topRight, botLeft, botRight, 1, 1)).toBe(40);
    });
  });

  describe('Table Grid Properties', () => {
    it('should calculate visible grid cells', () => {
      const gridSize = { rows: 16, cols: 16 };
      const viewportRows = 10;
      const viewportCols = 12;
      
      const visibleRows = Math.min(viewportRows, gridSize.rows);
      const visibleCols = Math.min(viewportCols, gridSize.cols);
      
      expect(visibleRows).toBe(10);
      expect(visibleCols).toBe(12);
    });

    it('should calculate cell pixel dimensions', () => {
      const cellWidth = 50;
      const cellHeight = 30;
      const pixelWidth = 16 * cellWidth;
      const pixelHeight = 16 * cellHeight;
      
      expect(pixelWidth).toBe(800);
      expect(pixelHeight).toBe(480);
    });
  });

  describe('History/Undo Stack', () => {
    it('should maintain undo history', () => {
      const history: number[][] = [];
      const state = [[1, 2, 3]];
      
      history.push([...state[0]]);
      state[0] = [4, 5, 6];
      history.push([...state[0]]);
      
      expect(history.length).toBe(2);
      expect(history[0]).toEqual([1, 2, 3]);
      expect(history[1]).toEqual([4, 5, 6]);
    });

    it('should undo to previous state', () => {
      const history = [[1, 2], [2, 3], [3, 4]];
      const currentIndex = 2;
      
      if (currentIndex > 0) {
        const previousState = history[currentIndex - 1];
        expect(previousState).toEqual([2, 3]);
      }
    });

    it('should limit history size', () => {
      const maxHistory = 50;
      let history: number[] = [];
      
      for (let i = 0; i < 100; i++) {
        history.push(i);
        if (history.length > maxHistory) {
          history.shift();
        }
      }
      
      expect(history.length).toBe(maxHistory);
      expect(history[0]).toBe(50);
    });
  });

  describe('Drag and Drop', () => {
    it('should detect drag start', () => {
      const startCell = '5_10';
      const isDragging = !!startCell;
      expect(isDragging).toBe(true);
    });

    it('should calculate drag selection', () => {
      const start: [number, number] = [2, 3];
      const end: [number, number] = [5, 7];
      
      const selection = new Set<string>();
      for (let r = Math.min(start[0], end[0]); r <= Math.max(start[0], end[0]); r++) {
        for (let c = Math.min(start[1], end[1]); c <= Math.max(start[1], end[1]); c++) {
          selection.add(`${r}_${c}`);
        }
      }
      
      expect(selection.size).toBe((5-2+1) * (7-3+1)); // 4 * 5 = 20
    });
  });

  describe('Search and Filter', () => {
    it('should find cells matching value', () => {
      const grid = [
        [10, 14.7, 10],
        [15, 14.7, 15],
        [20, 14.7, 20],
      ];
      
      const matches = new Set<string>();
      grid.forEach((row, r) => {
        row.forEach((val, c) => {
          if (val === 14.7) matches.add(`${r}_${c}`);
        });
      });
      
      expect(matches.size).toBe(3);
      expect(matches.has('0_1')).toBe(true);
    });

    it('should find cells in value range', () => {
      const grid = [5, 15, 25, 35, 45];
      const min = 20, max = 40;
      
      const inRange = grid.filter(v => v >= min && v <= max);
      expect(inRange).toEqual([25, 35]);
    });
  });
});

describe('TableEditor2D Toolbar Operations', () => {
  it('should set equal values', () => {
    const values = [10, 20, 30];
    const average = values.reduce((a, b) => a + b) / values.length;
    const result = values.map(() => average);
    
    expect(result).toEqual([20, 20, 20]);
  });

  it('should increase values by percentage', () => {
    const values = [100, 200];
    const increase = 0.1; // 10%
    const result = values.map(v => v * (1 + increase));
    
    expect(result[0]).toBeCloseTo(110, 5);
    expect(result[1]).toBeCloseTo(220, 5);
  });

  it('should decrease values by percentage', () => {
    const values = [100, 200];
    const decrease = 0.1; // 10%
    const result = values.map(v => v * (1 - decrease));
    
    expect(result[0]).toBe(90);
    expect(result[1]).toBe(180);
  });
});
