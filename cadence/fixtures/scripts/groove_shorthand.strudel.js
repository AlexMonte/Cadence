const bass = x => x.n("0 0 3 5").s("supersaw").lpf(800)
$: bass, s("bd").gain(0.7)
