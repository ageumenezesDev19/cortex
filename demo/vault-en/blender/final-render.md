# Final render settings

## Samples

Cycles at 128 samples with denoise on usually comes out clean. Past 256 the
gain is small and the time doubles.

## Resolution and ratio

Rendering at 200% of the final size and scaling down afterwards hides the jagged
edges better than any smoothing filter.

## What breaks in the middle of the night

Leaving a render running with the disk nearly full. Blender writes each frame as
it finishes, and when the space runs out it stops without warning and without
saving the sequence's progress.
