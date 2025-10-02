# EQ Toggle Fix - Apply During Pause

## Issue
When clicking the EQ "On" or "Off" buttons while audio was paused, the EQ changes would not be applied until the audio was played again. This was confusing for users who expected immediate application of EQ settings.

## Root Cause
The `setEQEnabled()` method only reconnected the audio chain when `isAudioPlaying` was `true`. However, when audio is paused:
- `isAudioPlaying` is `false`
- `isAudioPaused` is `true`
- The `audioSource` still exists but is suspended via `audioContext.suspend()`
- The audio graph remains connected with the old EQ configuration

Simply calling `connectAudioChain()` during pause doesn't work because the Web Audio API doesn't allow reconnecting nodes while the context is suspended. The audio source must be recreated with the new audio chain.

## Solution
Modified `setEQEnabled()` to handle three states:

1. **Audio is actively playing**: Reconnect the audio chain directly (original behavior)
2. **Audio is paused**: Restart from the current pause position with the new EQ settings
3. **Audio is stopped**: No action needed (EQ will be applied on next play)

### Implementation Details

#### 1. Added Pause Time Tracking
```typescript
private audioPauseTime: number = 0;
```

#### 2. Updated `pause()` Method
Save the current playback position when pausing:
```typescript
this.audioPauseTime = this.audioContext.currentTime - this.audioStartTime;
```

#### 3. Added Helper Methods

**`getCurrentTimeWhilePaused()`**: Returns the saved pause time when audio is paused
```typescript
private getCurrentTimeWhilePaused(): number {
  if (this.isAudioPaused) {
    return this.audioPauseTime;
  }
  return this.getCurrentTime();
}
```

**`restartFromPosition(startTime: number)`**: Recreates the audio source and restarts from a specific position
```typescript
private async restartFromPosition(startTime: number): Promise<void> {
  // 1. Resume context if suspended
  // 2. Stop and recreate audio source
  // 3. Reconnect audio chain with new EQ settings
  // 4. Start playback from saved position
  // 5. Immediately suspend to keep paused state
}
```

#### 4. Modified `setEQEnabled()`
Now handles pause state:
```typescript
if (this.audioSource) {
  if (this.isAudioPlaying) {
    // Audio is actively playing - reconnect chain directly
    this.connectAudioChain();
  } else if (this.isAudioPaused) {
    // Audio is paused - restart from current position with new EQ
    const currentTime = this.getCurrentTimeWhilePaused();
    this.restartFromPosition(currentTime);
  }
}
```

#### 5. Updated `resume()` and `stop()`
Clear the pause time when resuming or stopping to prevent stale state.

## Behavior After Fix

### Scenario 1: Toggle EQ while playing
- Audio continues playing without interruption
- EQ changes are applied immediately
- No audio glitches or position changes

### Scenario 2: Toggle EQ while paused
- Audio remains paused at the same position
- Audio source is recreated with new EQ configuration
- User can resume with the new EQ settings applied
- No audible glitch when toggling

### Scenario 3: Toggle EQ while stopped
- EQ state is updated
- Changes will apply when audio is played next

## Testing
1. Load a demo track
2. Play audio and click "Off" - verify EQ is removed immediately during playback
3. Click "On" - verify EQ is applied immediately during playback
4. Pause audio
5. Click "Off" - verify audio stays paused at same position
6. Resume - verify audio plays without EQ
7. Pause again, click "On" - verify audio stays paused
8. Resume - verify audio plays with EQ applied

## Technical Notes

### Why Restart is Necessary During Pause
The Web Audio API has limitations:
- `AudioBufferSourceNode` connections are fixed once set up
- You cannot disconnect and reconnect nodes while the context is suspended
- The only way to change the audio graph is to create a new source node

### Position Accuracy
The restart maintains accurate playback position by:
- Saving the exact time when pause occurs
- Using `AudioBufferSourceNode.start(when, offset)` to resume from the saved position
- Adjusting `audioStartTime` to maintain correct position tracking

### Edge Cases Handled
- Rapid EQ toggling during pause: Each toggle recreates the source with the latest settings
- Context state management: Properly handles suspended/running states
- Cleanup: Pause time is reset on resume and stop to prevent stale state

## Files Modified
- `/Users/pierrre/src.local/autoeq/src-ui/src/modules/audio/audio-player.ts`
  - Line 47: Added `audioPauseTime` instance variable
  - Line 786-829: Modified `setEQEnabled()` method
  - Line 1208-1214: Added `getCurrentTimeWhilePaused()` helper
  - Line 1216-1274: Added `restartFromPosition()` helper
  - Line 1357-1365: Updated `pause()` to save pause time
  - Line 1376-1385: Updated `resume()` to clear pause time
  - Line 1386-1408: Updated `stop()` to clear pause time

## Date
Applied: 2025-10-01
