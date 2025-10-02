# Canvas Clearing Fix for Audio Module

## Issue
When updating EQ parameters in the audio module, previous graph drawings in the mini graph display were not being fully cleared, causing visual artifacts and overlapping graphics.

## Root Cause
The canvas drawing functions were only using `fillRect()` to "clear" the canvas by drawing a filled rectangle over it. While this typically works, it doesn't properly reset the canvas state and can leave residual drawing artifacts, especially when:
- Multiple rapid updates occur
- Complex paths and shapes are drawn
- Canvas transformations are applied

## Solution
Added proper canvas clearing using `clearRect()` before redrawing in all canvas drawing functions:

1. **`drawEQMiniGraph()` (line 855)**: Mini EQ frequency response graph
2. **`drawIdleSpectrum()` (line 1036)**: Idle state spectrum visualization
3. **`startSpectrumAnalysis()` (line 1086)**: Active spectrum analyzer drawing loop

### Changes Made

#### Before:
```typescript
// Only filled with a background color
ctx.fillStyle = isDarkMode ? 'rgba(0, 0, 0, 0.2)' : 'rgba(255, 255, 255, 0.2)';
ctx.fillRect(0, 0, width, height);
```

#### After:
```typescript
// Properly clear canvas first, then fill background
ctx.clearRect(0, 0, width, height);

ctx.fillStyle = isDarkMode ? 'rgba(0, 0, 0, 0.2)' : 'rgba(255, 255, 255, 0.2)';
ctx.fillRect(0, 0, width, height);
```

## Benefits
- **Complete cleanup**: `clearRect()` fully erases all previous drawings and resets the canvas to a transparent state
- **State reset**: Ensures no leftover drawing state affects new graphics
- **Consistent behavior**: Works reliably across different browsers and canvas implementations
- **No artifacts**: Eliminates visual glitches from overlapping or incomplete clearing

## Testing
After applying this fix:
1. Open the audio player with EQ enabled
2. Load a demo track
3. Open the EQ configuration modal (⚙️ button)
4. Adjust multiple EQ filter parameters rapidly
5. Verify the mini graph updates cleanly without artifacts
6. Toggle EQ on/off and verify the graph displays correctly

## Files Modified
- `/Users/pierrre/src.local/autoeq/src-ui/src/modules/audio/audio-player.ts`
  - Line 855: `drawEQMiniGraph()` method
  - Line 1036: `drawIdleSpectrum()` method
  - Line 1086: Spectrum animation loop in `startSpectrumAnalysis()`

## Best Practice
Always use `clearRect()` before redrawing canvas content when:
- The entire canvas needs to be redrawn
- Previous drawings should not affect new drawings
- You want to ensure a clean slate for each frame

## Date
Applied: 2025-10-01
