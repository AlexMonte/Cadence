setcpm(132)
const drums = n("0 2 4 6").s("bd").struct("<x _ x _>")
const accents = s("sn").gain(0.45)
const layerA = layer(drums, accents)
const scene = arrange([4, layerA], [4, stack(accents, drums)])
scene
