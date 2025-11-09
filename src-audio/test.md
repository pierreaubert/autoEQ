Write a test framework to add the the current test.
The user provide a device that can do a audio loopback.

# Tests for plugins

# Tests for soft_audio play

- play a song, test that the player stop by itself
- play a song, record the result and compare
- play a song with a upmixer, validate that we have 5 channels in the ouput recorded file (asume you send to channels 0,1 and read from 0,1,2,3,4)
- play a song with an eq A and the same eq A but with negative gain and check the result

# Tests for soft_audio recording

