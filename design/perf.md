# voxel engine
- removed faces in between chunks
at 10 render distance it's 2,457,756 triangles and 486mbs of memory 330fps, it can mesh 7 chunks in 3.3ms avg~ compared to 47 fps 152,678,472 triangles 2.33gbs of memory and it could mesh 8 chunks in 3.3ms avg~
(3.3ms is the measurement unit cos you have 3ms of chunk remeshing budget)
10 render distance is 8000 chunks btw
that's 98% less triangles and 80% less memory
okay well this is the ideal conditions considering the world is flat but still


okay multi threading chunk generation made us go from 2330 chunks per second to 46,000 per second

skipping empty chunks while rendering (20 render distance 64k chunks) dropped rendering time from 40ms to 16ms

