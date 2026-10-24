import { describe, expect, it } from 'vitest';
import { getFileName, getParentDirectory } from './path';

describe('path utils', () => {
  describe('getParentDirectory', () => {
    it('extracts Unix parent directory correctly', () => {
      expect(getParentDirectory('/home/user/Downloads/video.mp4')).toBe('/home/user/Downloads');
    });

    it('extracts Windows parent directory correctly', () => {
      expect(getParentDirectory('C:\\Users\\User\\Downloads\\video.mp4')).toBe(
        'C:/Users/User/Downloads'
      );
    });

    it('handles trailing slashes gracefully', () => {
      expect(getParentDirectory('/home/user/Downloads/subfolder/')).toBe('/home/user/Downloads');
    });

    it('returns path when at top level or root', () => {
      expect(getParentDirectory('/video.mp4')).toBe('/video.mp4');
      expect(getParentDirectory('video.mp4')).toBe('video.mp4');
    });
  });

  describe('getFileName', () => {
    it('extracts Unix filename correctly', () => {
      expect(getFileName('/home/user/Downloads/video.mp4')).toBe('video.mp4');
    });

    it('extracts Windows filename correctly', () => {
      expect(getFileName('C:\\Users\\User\\Downloads\\music.mp3')).toBe('music.mp3');
    });

    it('handles files with spaces and special characters', () => {
      expect(getFileName('/downloads/My Awesome Video (1080p) [hd].mp4')).toBe(
        'My Awesome Video (1080p) [hd].mp4'
      );
    });

    it('returns bare filename when no directory given', () => {
      expect(getFileName('standalone.mkv')).toBe('standalone.mkv');
    });
  });
});
