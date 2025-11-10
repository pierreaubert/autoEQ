when i run
```
cargo run --release --bin sotf_player -- play ./src-tauri/public/demo-audio/female_vocal.wav --hwaudio-play "1,2->15,16"
```
the output says that we have 16 channels.

What I want is to generate only 2 channels.

the matrix should be like this

   1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
-+---------------------------------------
1| 0 0 0 0 0 0 0 0 0  0  0  0  0  0  1  0
-+---------------------------------------
2| 0 0 0 0 0 0 0 0 0  0  0  0  0  0  0  1
-+---------------------------------------

and they are only 2 non empty channels

==============================================

Redesign of the matrix plugin

it has 3 parts

input -> matrix -> permutation -> mute/solo
