const voxLine = n("0 2 4 7").s("vox").gain(0.72);
const rdLine = n("0 3 5 7").s("rd").struct("<[x _ x _]>").gain(0.66);
const ohLine = n("7 6 4 2").s("oh").fast(2).gain(0.58);

stack(voxLine, rdLine, ohLine)
