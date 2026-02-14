import { describe, it, expect } from 'vitest';
import { formatTokenCount } from '../format';

describe('formatTokenCount', () => {
  describe('zero and small numbers', () => {
    it('should return "0" for zero', () => {
      expect(formatTokenCount(0)).toBe('0');
    });

    it('should return the number as string for values 1-9', () => {
      expect(formatTokenCount(1)).toBe('1');
      expect(formatTokenCount(5)).toBe('5');
      expect(formatTokenCount(9)).toBe('9');
    });

    it('should return the number as string for values 10-99', () => {
      expect(formatTokenCount(10)).toBe('10');
      expect(formatTokenCount(50)).toBe('50');
      expect(formatTokenCount(99)).toBe('99');
    });

    it('should return the number as string for values 100-999', () => {
      expect(formatTokenCount(100)).toBe('100');
      expect(formatTokenCount(500)).toBe('500');
      expect(formatTokenCount(999)).toBe('999');
    });
  });

  describe('thousands (K values)', () => {
    it('should format exact thousands', () => {
      expect(formatTokenCount(1000)).toBe('1K');
      expect(formatTokenCount(5000)).toBe('5K');
      expect(formatTokenCount(10000)).toBe('10K');
    });

    it('should format fractional K values with one decimal place', () => {
      expect(formatTokenCount(1500)).toBe('1.5K');
      expect(formatTokenCount(2250)).toBe('2.2K');
      expect(formatTokenCount(9900)).toBe('9.9K');
    });

    it('should round fractional K values correctly', () => {
      expect(formatTokenCount(1450)).toBe('1.5K');
      expect(formatTokenCount(1440)).toBe('1.4K');
      expect(formatTokenCount(1455)).toBe('1.5K');
    });

    it('should format values just under 10K', () => {
      expect(formatTokenCount(9950)).toBe('10.0K');
      expect(formatTokenCount(9999)).toBe('10.0K');
    });

    it('should handle boundaries between K and M correctly', () => {
      expect(formatTokenCount(999500)).toBe('999.5K');
      expect(formatTokenCount(999950)).toBe('1000.0K');
    });
  });

  describe('millions (M values)', () => {
    it('should format exact millions', () => {
      expect(formatTokenCount(1000000)).toBe('1M');
      expect(formatTokenCount(5000000)).toBe('5M');
      expect(formatTokenCount(100000000)).toBe('100M');
    });

    it('should format fractional M values with one decimal place', () => {
      expect(formatTokenCount(1500000)).toBe('1.5M');
      expect(formatTokenCount(2250000)).toBe('2.2M');
      expect(formatTokenCount(9900000)).toBe('9.9M');
    });

    it('should round fractional M values correctly', () => {
      expect(formatTokenCount(1450000)).toBe('1.5M');
      expect(formatTokenCount(1440000)).toBe('1.4M');
      expect(formatTokenCount(1455000)).toBe('1.5M');
    });

    it('should handle very large numbers', () => {
      expect(formatTokenCount(1000000000)).toBe('1000M');
      expect(formatTokenCount(999999999)).toBe('1000.0M');
    });
  });

  describe('edge cases', () => {
    it('should handle negative numbers', () => {
      expect(formatTokenCount(-1)).toBe('-1');
      expect(formatTokenCount(-1000)).toBe('-1K');
      expect(formatTokenCount(-1500)).toBe('-1.5K');
      expect(formatTokenCount(-1000000)).toBe('-1M');
    });

    it('should handle NaN', () => {
      expect(formatTokenCount(NaN)).toBe('NaN');
    });

    it('should handle Infinity', () => {
      expect(formatTokenCount(Infinity)).toBe('Infinity');
    });

    it('should handle negative Infinity', () => {
      expect(formatTokenCount(-Infinity)).toBe('-Infinity');
    });

    it('should handle floating point numbers', () => {
      expect(formatTokenCount(1234.5)).toBe('1.2K');
      expect(formatTokenCount(1234567.8)).toBe('1.2M');
    });

    it('should handle very small positive numbers', () => {
      expect(formatTokenCount(0.1)).toBe('0.1');
      expect(formatTokenCount(0.5)).toBe('0.5');
    });
  });
});